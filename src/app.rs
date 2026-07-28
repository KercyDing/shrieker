use crate::settings;
use crate::tunnel::{self, HostStart, HostTask, HostUpdate};
use crate::ui;
use eframe::egui;
use sculk::minecraft::lan::{LanBroadcaster, LanScanner};
use sculk::persist::{Profile, TokenRefreshSetting};
use sculk::tunnel::{
    ConnectionSnapshot, HostedServiceStatus, JoinConfig, JoinOptions, JoinUri, LocalPort,
    SecretKey, TunnelEvent, TunnelPhase, TunnelService, TunnelStatus, TunnelUpdate,
};
use std::net::SocketAddr;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::mpsc;

const UPDATES_PER_FRAME_MAX: usize = 256;
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const TRAY_EVENTS_PER_FRAME_MAX: usize = 8;

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Mode {
    Host,
    Join,
    Settings,
    Preferences,
}

pub struct App {
    rt: tokio::runtime::Runtime,
    join_service: TunnelService,
    repaint: egui::Context,
    tray: Option<crate::tray::Tray>,
    tray_ready: bool,
    close_to_tray: bool,
    exit_requested: bool,
    join_rx: mpsc::UnboundedReceiver<TunnelUpdate>,
    join_stop_tx: mpsc::UnboundedSender<Result<(), String>>,
    join_stop_rx: mpsc::UnboundedReceiver<Result<(), String>>,
    host_task: Option<HostTask>,
    host_rx: mpsc::UnboundedReceiver<HostUpdate>,
    host_update_tx: mpsc::UnboundedSender<HostUpdate>,
    host_scan_stop_tx: mpsc::UnboundedSender<Result<(), String>>,
    host_scan_stop_rx: mpsc::UnboundedReceiver<Result<(), String>>,
    host_scanner: Option<LanScanner>,
    host_scanner_stopping: bool,
    lan_broadcaster: Option<LanBroadcaster>,
    stop_pending: bool,
    rotate_pending: bool,
    phase: TunnelPhase,
    active_mode: Option<Mode>,
    persisted_preferences: settings::GuiPreferences,
    pub(crate) mode: Mode,
    pub(crate) detected_mc_port: Option<NonZeroU16>,
    pub(crate) host_manual_port: bool,
    pub(crate) host_port: String,
    pub(crate) max_players: String,
    pub(crate) token_refresh: TokenRefreshSetting,
    pub(crate) join_uri_input: String,
    pub(crate) join_uri_select_all: bool,
    pub(crate) join_port: String,
    pub(crate) join_auto_port: bool,
    pub(crate) share_uri: Option<String>,
    pub(crate) logs: Vec<String>,
    pub(crate) profile: Profile,
    pub(crate) secret_key: Option<SecretKey>,
    pub(crate) relay_custom: bool,
    pub(crate) relay_url: String,
    pub(crate) reconnect_unlimited: bool,
    pub(crate) reconnect_timeout_secs: u64,
    pub(crate) remember_window_state: bool,
    pub(crate) close_action: settings::CloseAction,
    pub(crate) tunnel: TunnelStatus,
    pub(crate) host_status: Option<HostedServiceStatus>,
    pub(crate) theme_preference: egui::ThemePreference,
}

impl App {
    pub fn new(rt: tokio::runtime::Runtime, repaint: egui::Context, close_to_tray: bool) -> Self {
        let loaded = settings::load();
        rust_i18n::set_locale(&loaded.preferences.locale);
        let persisted_preferences = loaded.preferences.clone();

        let join_service = TunnelService::new();
        let tunnel = join_service.status();
        let join_rx = tunnel::subscribe_join(&rt, join_service.subscribe(), repaint.clone());
        let (join_stop_tx, join_stop_rx) = mpsc::unbounded_channel();
        let (host_update_tx, host_rx) = mpsc::unbounded_channel();
        let (host_scan_stop_tx, host_scan_stop_rx) = mpsc::unbounded_channel();

        let mut logs = loaded.errors;
        if logs.is_empty() {
            logs.push(t!("profile_loaded").to_string());
        }
        let tray = if close_to_tray {
            match crate::tray::Tray::new(repaint.clone()) {
                Ok(tray) => Some(tray),
                Err(error) => {
                    logs.push(t!("tray_failed", err = error).to_string());
                    None
                }
            }
        } else {
            None
        };
        let host_scanner = match LanScanner::start() {
            Ok(scanner) => Some(scanner),
            Err(error) => {
                logs.push(t!("lan_scan_failed", err = error).to_string());
                None
            }
        };

        Self {
            rt,
            join_service,
            repaint,
            tray,
            tray_ready: false,
            close_to_tray,
            exit_requested: false,
            join_rx,
            join_stop_tx,
            join_stop_rx,
            host_task: None,
            host_rx,
            host_update_tx,
            host_scan_stop_tx,
            host_scan_stop_rx,
            host_scanner,
            host_scanner_stopping: false,
            lan_broadcaster: None,
            stop_pending: false,
            rotate_pending: false,
            phase: TunnelPhase::Idle,
            active_mode: None,
            persisted_preferences,
            mode: Mode::Host,
            detected_mc_port: None,
            host_manual_port: false,
            host_port: loaded.profile.host.port.to_string(),
            max_players: String::new(),
            token_refresh: loaded.profile.host.token_refresh,
            join_uri_input: loaded.preferences.join_uri.clone(),
            join_uri_select_all: false,
            join_port: loaded.profile.join.port.to_string(),
            join_auto_port: true,
            share_uri: None,
            relay_custom: loaded.profile.relay.custom,
            relay_url: loaded.profile.relay.url.clone().unwrap_or_default(),
            reconnect_unlimited: loaded.preferences.reconnect_timeout_secs.is_none(),
            reconnect_timeout_secs: loaded
                .preferences
                .reconnect_timeout_secs
                .unwrap_or(settings::DEFAULT_RECONNECT_TIMEOUT_SECS),
            remember_window_state: loaded.preferences.remember_window_state,
            close_action: loaded.preferences.close_action,
            profile: loaded.profile,
            secret_key: loaded.secret_key,
            logs,
            tunnel,
            host_status: None,
            theme_preference: Self::parse_theme_preference(&loaded.preferences.theme),
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.phase == TunnelPhase::Idle
    }

    pub(crate) fn phase(&self) -> TunnelPhase {
        self.phase
    }

    pub(crate) fn stop_pending(&self) -> bool {
        self.stop_pending
    }

    pub(crate) fn rotate_pending(&self) -> bool {
        self.rotate_pending
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode || !self.is_idle() {
            return;
        }
        self.mode = mode;
        if mode == Mode::Join {
            self.join_uri_select_all = true;
        }
        if mode == Mode::Host {
            self.start_host_scan();
        } else {
            self.stop_host_scan();
        }
    }

    pub(crate) fn join_local_addr(&self) -> Option<SocketAddr> {
        (self.active_mode == Some(Mode::Join))
            .then_some(self.tunnel.state.local_addr)
            .flatten()
    }

    pub(crate) fn join_connections(&self) -> &[ConnectionSnapshot] {
        if self.active_mode == Some(Mode::Join) {
            &self.tunnel.connections
        } else {
            &[]
        }
    }

    pub(crate) fn save_host_token_refresh(&mut self) {
        self.profile.host.token_refresh = self.token_refresh;
        self.persist_profile();
    }

    pub(crate) fn save_host_port(&mut self) {
        if let Ok(port) = parse_host_port(&self.host_port) {
            self.profile.host.port = port.get();
            self.persist_profile();
        }
    }

    pub(crate) fn host_port_ready(&self) -> bool {
        if self.host_manual_port {
            parse_host_port(&self.host_port).is_ok()
        } else {
            self.detected_mc_port.is_some()
        }
    }

    pub(crate) fn save_join_port(&mut self) {
        if let Ok(port) = self.join_port.parse() {
            self.profile.join.port = port;
            self.persist_profile();
        }
    }

    fn persist_profile(&mut self) {
        if let Err(error) = settings::save_profile(&self.profile) {
            self.logs
                .push(t!("save_profile_err", err = error).to_string());
        }
    }

    pub(crate) fn start_host(&mut self) {
        let mc_port = if self.host_manual_port {
            match parse_host_port(&self.host_port) {
                Ok(port) => port.get(),
                Err(error) => {
                    self.logs.push(t!("host_failed", err = error).to_string());
                    return;
                }
            }
        } else {
            let Some(port) = self.detected_mc_port else {
                self.logs.push(t!("mc_server_unavailable").to_string());
                return;
            };
            port.get()
        };
        let max_players = match parse_optional_u32(&self.max_players) {
            Ok(value) => value,
            Err(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                return;
            }
        };
        let Some(secret_key) = self.secret_key.clone() else {
            self.logs
                .push(t!("host_failed", err = "persistent key unavailable").to_string());
            return;
        };
        let relay_url = match self.profile.resolve_relay_url(None) {
            Ok(relay_url) => relay_url,
            Err(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                return;
            }
        };
        let state_path = match settings::host_state_path() {
            Ok(path) => path,
            Err(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                return;
            }
        };

        let start = HostStart {
            mc_port,
            max_players,
            secret_key,
            relay_url,
            token_refresh: tunnel::token_refresh_policy(self.token_refresh),
            state_path,
        };
        self.host_task = Some(HostTask::spawn(
            &self.rt,
            start,
            self.host_update_tx.clone(),
            self.repaint.clone(),
        ));
        self.stop_host_scan();
        self.phase = TunnelPhase::Starting;
        self.active_mode = Some(Mode::Host);
        self.logs.push(t!("starting_tunnel").to_string());
    }

    pub(crate) fn rotate_host_uri(&mut self) {
        if self.rotate_pending || self.phase != TunnelPhase::Active {
            return;
        }
        let Some(host_task) = &self.host_task else {
            return;
        };
        if host_task.rotate() {
            self.rotate_pending = true;
            self.logs.push(t!("refreshing_share_uri").to_string());
        }
    }

    pub(crate) fn start_join(&mut self) {
        let join_uri = match self.join_uri_input.trim().parse::<JoinUri>() {
            Ok(join_uri) => join_uri,
            Err(error) => {
                self.logs
                    .push(t!("invalid_share_uri", err = error).to_string());
                return;
            }
        };
        let local_port = match parse_local_port(self.join_auto_port, &self.join_port) {
            Ok(port) => port,
            Err(error) => {
                self.logs.push(t!("join_failed", err = error).to_string());
                return;
            }
        };
        let options = JoinOptions::new(join_uri)
            .local_port(local_port)
            .config(self.join_config());

        self.logs.push(t!("joining_tunnel").to_string());
        if let Err(error) = self.rt.block_on(self.join_service.start_join(options)) {
            self.logs.push(t!("join_failed", err = error).to_string());
            return;
        }
        self.active_mode = Some(Mode::Join);
        self.tunnel = self.join_service.status();
        self.phase = self.tunnel.state.phase;
    }

    pub(crate) fn stop(&mut self) {
        if self.stop_pending || self.is_idle() {
            return;
        }
        self.stop_pending = true;

        if self.active_mode == Some(Mode::Host) {
            let sent = self.host_task.as_ref().is_some_and(|task| task.stop(None));
            if !sent {
                self.stop_pending = false;
                self.logs
                    .push(t!("stop_failed", err = "host task unavailable").to_string());
            }
            return;
        }

        self.stop_lan_broadcast();
        let service = self.join_service.clone();
        let tx = self.join_stop_tx.clone();
        let repaint = self.repaint.clone();
        self.rt.spawn(async move {
            let result = service.shutdown().await.map_err(|error| error.to_string());
            let _ = tx.send(result);
            repaint.request_repaint();
        });
    }

    fn poll(&mut self) {
        self.poll_host_scan();
        for _ in 0..UPDATES_PER_FRAME_MAX {
            let Ok(update) = self.join_rx.try_recv() else {
                break;
            };
            self.apply_join_update(update);
        }
        for _ in 0..UPDATES_PER_FRAME_MAX {
            let Ok(update) = self.host_rx.try_recv() else {
                break;
            };
            self.apply_host_update(update);
        }
        if let Ok(result) = self.host_scan_stop_rx.try_recv() {
            self.host_scanner_stopping = false;
            if let Err(error) = result {
                self.logs
                    .push(t!("lan_scan_failed", err = error).to_string());
            }
            self.start_host_scan();
        }
        if let Ok(Err(error)) = self.join_stop_rx.try_recv() {
            self.stop_pending = false;
            self.logs.push(t!("stop_failed", err = error).to_string());
            let status = self.join_service.status();
            if self.active_mode == Some(Mode::Join)
                && status.state.phase == TunnelPhase::Active
                && let Some(addr) = status.state.local_addr
            {
                self.start_lan_broadcast(addr.port());
            }
        }
        self.poll_lan_broadcast();
    }

    fn apply_join_update(&mut self, update: TunnelUpdate) {
        match update {
            TunnelUpdate::Status(status) => self.apply_join_status(status),
            TunnelUpdate::Event(event) => self.apply_join_event(event),
            _ => {}
        }
    }

    fn apply_join_event(&mut self, event: TunnelEvent) {
        if self.active_mode == Some(Mode::Join) {
            match &event {
                TunnelEvent::Disconnected { .. } | TunnelEvent::Reconnecting { .. } => {
                    self.stop_lan_broadcast();
                }
                TunnelEvent::Reconnected
                    if self.phase == TunnelPhase::Active && !self.stop_pending =>
                {
                    if let Some(addr) = self.tunnel.state.local_addr {
                        self.start_lan_broadcast(addr.port());
                    }
                }
                _ => {}
            }
        }
        let log = match &event {
            TunnelEvent::Reconnecting { attempt } if !self.reconnect_unlimited => t!(
                "reconnecting_limited",
                n = attempt,
                max = self.reconnect_timeout_secs
            )
            .to_string(),
            _ => tunnel::format_event(&event),
        };
        self.logs.push(log);
    }

    fn apply_join_status(&mut self, status: TunnelStatus) {
        if self.active_mode != Some(Mode::Join) && status.state.phase == TunnelPhase::Idle {
            self.tunnel = status;
            return;
        }

        let previous = self.phase;
        self.phase = status.state.phase;
        if previous != TunnelPhase::Active && self.phase == TunnelPhase::Active {
            self.persisted_preferences.join_uri = self.join_uri_input.trim().to_owned();
            self.persist_preferences();
            if let Some(addr) = status.state.local_addr {
                if !self.stop_pending {
                    self.start_lan_broadcast(addr.port());
                }
                self.logs.push(t!("joined_at", addr = addr).to_string());
            } else {
                self.logs.push(t!("joined").to_string());
            }
        } else if previous != TunnelPhase::Idle && self.phase == TunnelPhase::Idle {
            if previous == TunnelPhase::Starting && !self.stop_pending {
                self.stop_lan_broadcast();
                self.active_mode = None;
            } else {
                self.finish_stopped();
            }
        }
        self.tunnel = status;
    }

    fn start_lan_broadcast(&mut self, port: u16) {
        self.stop_lan_broadcast();
        let Some(port) = NonZeroU16::new(port) else {
            self.logs
                .push(t!("lan_broadcast_failed", err = "local port is zero").to_string());
            return;
        };
        match LanBroadcaster::start("shrieker", port) {
            Ok(broadcaster) => {
                self.lan_broadcaster = Some(broadcaster);
                self.logs.push(t!("lan_broadcast_started").to_string());
            }
            Err(error) => self
                .logs
                .push(t!("lan_broadcast_failed", err = error).to_string()),
        }
    }

    fn stop_lan_broadcast(&mut self) {
        let Some(broadcaster) = self.lan_broadcaster.take() else {
            return;
        };
        if let Err(error) = broadcaster.stop() {
            self.logs
                .push(t!("lan_broadcast_failed", err = error).to_string());
        }
    }

    fn poll_lan_broadcast(&mut self) {
        if !self
            .lan_broadcaster
            .as_ref()
            .is_some_and(LanBroadcaster::is_finished)
        {
            return;
        }
        self.stop_lan_broadcast();
    }

    fn start_host_scan(&mut self) {
        if self.host_scanner.is_some()
            || self.host_scanner_stopping
            || self.detected_mc_port.is_some()
            || self.mode != Mode::Host
            || !self.is_idle()
        {
            return;
        }
        match LanScanner::start() {
            Ok(scanner) => self.host_scanner = Some(scanner),
            Err(error) => self
                .logs
                .push(t!("lan_scan_failed", err = error).to_string()),
        }
    }

    fn stop_host_scan(&mut self) {
        let Some(scanner) = self.host_scanner.take() else {
            return;
        };
        self.host_scanner_stopping = true;
        let stop_tx = self.host_scan_stop_tx.clone();
        let repaint = self.repaint.clone();
        self.rt.spawn_blocking(move || {
            let result = scanner.stop().map_err(|error| error.to_string());
            let _ = stop_tx.send(result);
            repaint.request_repaint();
        });
    }

    fn poll_host_scan(&mut self) {
        let (detected, finished) = {
            let Some(scanner) = &self.host_scanner else {
                return;
            };
            let mut detected = None;
            while let Ok(port) = scanner.try_recv() {
                detected = Some(port);
            }
            (detected, scanner.is_finished())
        };

        if let Some(port) = detected {
            self.detected_mc_port = Some(port);
            if !self.host_manual_port {
                self.host_port = port.to_string();
                self.profile.host.port = port.get();
                self.persist_profile();
            }
        }
        if !finished {
            return;
        }
        let Some(scanner) = self.host_scanner.take() else {
            return;
        };
        if let Err(error) = scanner.stop() {
            self.logs
                .push(t!("lan_scan_failed", err = error).to_string());
        }
    }

    fn apply_host_update(&mut self, update: HostUpdate) {
        match update {
            HostUpdate::Started { uri, status } => {
                self.phase = TunnelPhase::Active;
                self.host_status = Some(status);
                self.set_share_uri(uri, false);
                self.logs.push(t!("host_ready").to_string());
            }
            HostUpdate::Status(status) => self.host_status = Some(status),
            HostUpdate::UriChanged { uri, status } => {
                self.rotate_pending = false;
                self.host_status = Some(status);
                self.set_share_uri(uri, true);
            }
            HostUpdate::Event(event) => self.logs.push(tunnel::format_event(&event)),
            HostUpdate::Error(error) => {
                self.rotate_pending = false;
                self.logs.push(t!("error_msg", msg = error).to_string());
            }
            HostUpdate::MinecraftUnavailable => {
                self.detected_mc_port = None;
                self.logs.push(t!("mc_server_unavailable").to_string());
                self.finish_host();
            }
            HostUpdate::Failed(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                self.finish_host();
            }
            HostUpdate::Stopped(result) => {
                if let Err(error) = result {
                    self.logs.push(t!("stop_failed", err = error).to_string());
                } else {
                    self.logs.push(t!("tunnel_closed").to_string());
                }
                self.finish_host();
            }
        }
    }

    fn set_share_uri(&mut self, uri: String, refreshed: bool) {
        self.repaint.copy_text(uri.clone());
        self.share_uri = Some(uri);
        let message = if refreshed {
            t!("share_uri_refreshed")
        } else {
            t!("share_uri_copied")
        };
        self.logs.push(message.to_string());
    }

    fn finish_stopped(&mut self) {
        self.stop_lan_broadcast();
        self.logs.push(t!("tunnel_closed").to_string());
        self.stop_pending = false;
        self.phase = TunnelPhase::Idle;
        self.active_mode = None;
    }

    fn finish_host(&mut self) {
        self.stop_pending = false;
        self.rotate_pending = false;
        self.phase = TunnelPhase::Idle;
        self.active_mode = None;
        self.host_task = None;
        self.host_status = None;
        self.share_uri = None;
        self.start_host_scan();
    }

    pub(crate) fn set_language(&mut self, locale: &str) {
        if &*rust_i18n::locale() != locale {
            rust_i18n::set_locale(locale);
        }
    }

    pub(crate) fn set_theme(&mut self, theme: egui::ThemePreference, ctx: &egui::Context) {
        if self.theme_preference == theme {
            return;
        }
        self.theme_preference = theme;
        ctx.set_theme(theme);
    }

    pub(crate) fn set_remember_window_state(&mut self, remember: bool) {
        self.remember_window_state = remember;
    }

    pub(crate) fn set_close_action(&mut self, action: settings::CloseAction) {
        self.close_action = action;
    }

    pub(crate) fn save_settings(&mut self) {
        self.profile.relay.custom = self.relay_custom;
        self.profile.relay.url = (!self.relay_url.is_empty()).then(|| self.relay_url.clone());
        self.persist_profile();
        self.persisted_preferences.reconnect_timeout_secs =
            (!self.reconnect_unlimited).then_some(self.reconnect_timeout_secs);
        self.persist_preferences();
        self.logs.push(t!("settings_saved").to_string());
    }

    pub(crate) fn save_preference_settings(&mut self) {
        self.persisted_preferences.theme = Self::theme_name(self.theme_preference).to_owned();
        self.persisted_preferences.locale = rust_i18n::locale().to_string();
        self.persisted_preferences.remember_window_state = self.remember_window_state;
        self.persisted_preferences.close_action = self.close_action;
        self.persist_preferences();
        self.logs.push(t!("preferences_saved").to_string());
    }

    fn persist_preferences(&mut self) {
        if let Err(error) = settings::save_preferences(&self.persisted_preferences) {
            self.logs
                .push(t!("save_preferences_err", err = error).to_string());
        }
    }

    fn join_config(&self) -> JoinConfig {
        let mut config = JoinConfig::new();
        config.reconnect_timeout =
            (!self.reconnect_unlimited).then_some(Duration::from_secs(self.reconnect_timeout_secs));
        config
    }

    fn parse_theme_preference(value: &str) -> egui::ThemePreference {
        match value {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        }
    }

    fn theme_name(theme: egui::ThemePreference) -> &'static str {
        match theme {
            egui::ThemePreference::System => "system",
            egui::ThemePreference::Light => "light",
            egui::ThemePreference::Dark => "dark",
        }
    }

    fn shutdown_on_exit(&mut self) {
        self.stop_host_scan();
        self.stop_lan_broadcast();
        if let Some(host_task) = self.host_task.take() {
            let (done_tx, done_rx) = std_mpsc::channel();
            if host_task.stop(Some(done_tx)) {
                let _ = done_rx.recv_timeout(EXIT_TIMEOUT);
            }
        }
        let service = self.join_service.clone();
        let (done_tx, done_rx) = std_mpsc::channel();
        self.rt.spawn(async move {
            let _ = service.shutdown().await;
            let _ = done_tx.send(());
        });
        let _ = done_rx.recv_timeout(EXIT_TIMEOUT);
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "linux")]
        let mut tray_failed = false;
        for _ in 0..TRAY_EVENTS_PER_FRAME_MAX {
            let Some(event) = self.tray.as_ref().and_then(crate::tray::Tray::try_recv) else {
                break;
            };
            match event {
                crate::tray::Event::Ready => {
                    self.tray_ready = true;
                }
                crate::tray::Event::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::tray::Event::Exit => {
                    self.exit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                #[cfg(target_os = "linux")]
                crate::tray::Event::Failed(error) => {
                    self.logs.push(t!("tray_failed", err = error).to_string());
                    self.tray_ready = false;
                    tray_failed = true;
                }
            }
        }
        #[cfg(target_os = "linux")]
        if tray_failed {
            self.tray = None;
        }
    }

    fn handle_close_request(&self, ctx: &egui::Context) {
        let close_requested = ctx.input(|input| input.viewport().close_requested());
        if !should_hide_on_close(
            close_requested,
            self.exit_requested,
            self.close_action,
            self.close_to_tray && self.tray_ready,
        ) {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
}

fn should_hide_on_close(
    close_requested: bool,
    exit_requested: bool,
    close_action: settings::CloseAction,
    tray_available: bool,
) -> bool {
    close_requested
        && !exit_requested
        && close_action == settings::CloseAction::HideToTray
        && tray_available
}

fn parse_local_port(auto: bool, value: &str) -> Result<LocalPort, std::num::ParseIntError> {
    if auto {
        Ok(LocalPort::Auto)
    } else {
        value.parse::<NonZeroU16>().map(LocalPort::Fixed)
    }
}

fn parse_host_port(value: &str) -> Result<NonZeroU16, std::num::ParseIntError> {
    value.parse()
}

fn parse_optional_u32(value: &str) -> Result<Option<u32>, std::num::ParseIntError> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse::<NonZeroU32>().map(|value| Some(value.get()))
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_tray(ctx);
        self.handle_close_request(ctx);
        self.poll();

        if self.stop_pending
            || self.rotate_pending
            || !self.is_idle()
            || (self.mode == Mode::Host && self.host_scanner.is_some())
            || self.host_scanner_stopping
        {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::render(self, root);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown_on_exit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_optional_number() {
        assert_eq!(parse_optional_u32(""), Ok(None));
    }

    #[test]
    fn parses_present_optional_number() {
        assert_eq!(parse_optional_u32("8"), Ok(Some(8)));
    }

    #[test]
    fn rejects_invalid_optional_number() {
        assert!(parse_optional_u32("eight").is_err());
        assert!(parse_optional_u32("0").is_err());
    }

    #[test]
    fn uses_auto_local_port_without_parsing_input() {
        assert_eq!(parse_local_port(true, "invalid"), Ok(LocalPort::Auto));
    }

    #[test]
    fn parses_fixed_local_port() {
        assert!(matches!(
            parse_local_port(false, "30000"),
            Ok(LocalPort::Fixed(port)) if port.get() == 30000
        ));
        assert!(parse_local_port(false, "0").is_err());
    }

    #[test]
    fn parses_minecraft_host_port() {
        assert_eq!(parse_host_port("25565").map(NonZeroU16::get), Ok(25_565));
        assert!(parse_host_port("0").is_err());
        assert!(parse_host_port("65536").is_err());
        assert!(parse_host_port("invalid").is_err());
    }

    #[test]
    fn hides_only_for_user_close_with_available_tray() {
        assert!(should_hide_on_close(
            true,
            false,
            settings::CloseAction::HideToTray,
            true
        ));
        assert!(!should_hide_on_close(
            true,
            false,
            settings::CloseAction::Exit,
            true
        ));
        assert!(!should_hide_on_close(
            true,
            true,
            settings::CloseAction::HideToTray,
            true
        ));
        assert!(!should_hide_on_close(
            true,
            false,
            settings::CloseAction::HideToTray,
            false
        ));
        assert!(!should_hide_on_close(
            false,
            false,
            settings::CloseAction::HideToTray,
            true
        ));
    }
}
