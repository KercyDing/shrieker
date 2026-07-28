use crate::settings;
use crate::ui;
use eframe::egui;
use sculk::ErrorCategory;
use sculk::minecraft::lan::{LanBroadcaster, LanScanner};
use sculk::minecraft::probe_server;
use sculk::persist::{self, HostState, Profile, TokenRefreshSetting};
use sculk::tunnel::{
    ConnectionSnapshot, HostConfig, HostedServiceHandle, HostedServiceOptions, HostedServiceStatus,
    JoinConfig, JoinOptions, JoinUri, LocalPort, NodeOptions, SculkNode, SecretKey, ServiceId,
    TokenRefreshPolicy, TunnelEvent, TunnelPhase, TunnelService, TunnelStatus, TunnelUpdate,
};
use std::net::{Ipv4Addr, SocketAddr};
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

const UPDATES_PER_FRAME_MAX: usize = 256;
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const HOST_START_TIMEOUT: Duration = Duration::from_secs(15);
const MC_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const MC_HEALTH_INTERVAL: Duration = Duration::from_secs(5);
const MC_HEALTH_FAILURES_MAX: u8 = 3;

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Mode {
    Host,
    Join,
    Settings,
    Preferences,
}

enum HostCommand {
    Rotate,
    Stop { done: Option<std_mpsc::Sender<()>> },
}

enum HostUpdate {
    Started {
        uri: String,
        status: HostedServiceStatus,
    },
    Status(HostedServiceStatus),
    UriChanged {
        uri: String,
        status: HostedServiceStatus,
    },
    Event(TunnelEvent),
    Error(String),
    ScannerStopped(Result<(), String>),
    MinecraftUnavailable,
    Failed(String),
    Stopped(Result<(), String>),
}

struct HostStart {
    mc_port: u16,
    max_players: Option<u32>,
    secret_key: SecretKey,
    relay_url: Option<sculk::tunnel::RelayUrl>,
    token_refresh: TokenRefreshPolicy,
    state_path: PathBuf,
}

pub struct App {
    rt: tokio::runtime::Runtime,
    join_service: TunnelService,
    repaint: egui::Context,
    join_rx: mpsc::UnboundedReceiver<TunnelUpdate>,
    join_stop_tx: mpsc::UnboundedSender<Result<(), String>>,
    join_stop_rx: mpsc::UnboundedReceiver<Result<(), String>>,
    host_tx: Option<mpsc::UnboundedSender<HostCommand>>,
    host_rx: mpsc::UnboundedReceiver<HostUpdate>,
    host_update_tx: mpsc::UnboundedSender<HostUpdate>,
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
    pub(crate) reconnect_max_retries: u32,
    pub(crate) reconnect_interval_secs: u64,
    pub(crate) remember_window_state: bool,
    pub(crate) tunnel: TunnelStatus,
    pub(crate) host_status: Option<HostedServiceStatus>,
    pub(crate) theme_preference: egui::ThemePreference,
}

impl App {
    pub fn new(rt: tokio::runtime::Runtime, repaint: egui::Context) -> Self {
        let loaded = settings::load();
        rust_i18n::set_locale(&loaded.preferences.locale);
        let persisted_preferences = loaded.preferences.clone();

        let join_service = TunnelService::new();
        let tunnel = join_service.status();
        let join_rx = spawn_join_subscription(&rt, join_service.subscribe(), repaint.clone());
        let (join_stop_tx, join_stop_rx) = mpsc::unbounded_channel();
        let (host_update_tx, host_rx) = mpsc::unbounded_channel();

        let mut logs = loaded.errors;
        if logs.is_empty() {
            logs.push(t!("profile_loaded").to_string());
        }
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
            join_rx,
            join_stop_tx,
            join_stop_rx,
            host_tx: None,
            host_rx,
            host_update_tx,
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
            reconnect_unlimited: loaded.preferences.reconnect_max_retries.is_none(),
            reconnect_max_retries: loaded
                .preferences
                .reconnect_max_retries
                .unwrap_or(settings::DEFAULT_RECONNECT_MAX_RETRIES),
            reconnect_interval_secs: loaded.preferences.reconnect_interval_secs,
            remember_window_state: loaded.preferences.remember_window_state,
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
            token_refresh: token_refresh_policy(self.token_refresh),
            state_path,
        };
        let (host_tx, host_rx) = mpsc::unbounded_channel();
        self.host_tx = Some(host_tx);
        self.stop_host_scan();
        self.phase = TunnelPhase::Starting;
        self.active_mode = Some(Mode::Host);
        self.logs.push(t!("starting_tunnel").to_string());

        let update_tx = self.host_update_tx.clone();
        let repaint = self.repaint.clone();
        self.rt.spawn(async move {
            run_host(start, host_rx, update_tx).await;
            repaint.request_repaint();
        });
    }

    pub(crate) fn rotate_host_uri(&mut self) {
        if self.rotate_pending || self.phase != TunnelPhase::Active {
            return;
        }
        let Some(host_tx) = &self.host_tx else {
            return;
        };
        if host_tx.send(HostCommand::Rotate).is_ok() {
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
            let sent = self
                .host_tx
                .as_ref()
                .is_some_and(|tx| tx.send(HostCommand::Stop { done: None }).is_ok());
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
        self.logs.push(format_event(&event));
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
        let updates = self.host_update_tx.clone();
        let repaint = self.repaint.clone();
        self.rt.spawn_blocking(move || {
            let result = scanner.stop().map_err(|error| error.to_string());
            let _ = updates.send(HostUpdate::ScannerStopped(result));
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
            HostUpdate::Event(event) => self.logs.push(format_event(&event)),
            HostUpdate::Error(error) => {
                self.rotate_pending = false;
                self.logs.push(t!("error_msg", msg = error).to_string());
            }
            HostUpdate::ScannerStopped(result) => {
                self.host_scanner_stopping = false;
                if let Err(error) = result {
                    self.logs
                        .push(t!("lan_scan_failed", err = error).to_string());
                }
                self.start_host_scan();
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
        self.host_tx = None;
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

    pub(crate) fn save_settings(&mut self) {
        self.profile.relay.custom = self.relay_custom;
        self.profile.relay.url = (!self.relay_url.is_empty()).then(|| self.relay_url.clone());
        self.persist_profile();
        self.persisted_preferences.reconnect_max_retries =
            (!self.reconnect_unlimited).then_some(self.reconnect_max_retries);
        self.persisted_preferences.reconnect_interval_secs = self.reconnect_interval_secs;
        self.persist_preferences();
        self.logs.push(t!("settings_saved").to_string());
    }

    pub(crate) fn save_preference_settings(&mut self) {
        self.persisted_preferences.theme = Self::theme_name(self.theme_preference).to_owned();
        self.persisted_preferences.locale = rust_i18n::locale().to_string();
        self.persisted_preferences.remember_window_state = self.remember_window_state;
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
        config.max_retries = (!self.reconnect_unlimited).then_some(self.reconnect_max_retries);
        let reconnect_interval = Duration::from_secs(self.reconnect_interval_secs);
        config.base_backoff = reconnect_interval;
        config.max_backoff = reconnect_interval;
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
        if let Some(host_tx) = self.host_tx.take() {
            let (done_tx, done_rx) = std_mpsc::channel();
            if host_tx
                .send(HostCommand::Stop {
                    done: Some(done_tx),
                })
                .is_ok()
            {
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
}

async fn run_host(
    start: HostStart,
    mut commands: mpsc::UnboundedReceiver<HostCommand>,
    updates: mpsc::UnboundedSender<HostUpdate>,
) {
    let target_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, start.mc_port));
    if !minecraft_available(target_addr).await {
        let _ = updates.send(HostUpdate::MinecraftUnavailable);
        return;
    }
    let saved = match persist::load_host_state(&start.state_path) {
        Ok(saved) => saved,
        Err(error) => {
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let service_id = saved
        .as_ref()
        .map_or_else(ServiceId::generate, |state| state.service_id);
    let token_state = saved.map(|state| state.token_state);
    let node_options = NodeOptions {
        secret_key: Some(start.secret_key),
        relay_url: start.relay_url,
        ..NodeOptions::default()
    };
    let node = match tokio::time::timeout(HOST_START_TIMEOUT, SculkNode::bind(node_options)).await {
        Ok(Ok(node)) => node,
        Ok(Err(error)) => {
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
        Err(_) => {
            let _ = updates.send(HostUpdate::Failed(
                "node startup timed out; check relay settings".to_owned(),
            ));
            return;
        }
    };
    let host = match node
        .start_service(HostedServiceOptions {
            service_id,
            target_addr,
            token_state,
            token_refresh: start.token_refresh,
            config: HostConfig::new().max_players(start.max_players),
        })
        .await
    {
        Ok(host) => host,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let mut events = match host.subscribe().await {
        Ok(events) => events,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let mut statuses = match host.subscribe_status().await {
        Ok(statuses) => statuses,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    if let Err(error) = persist_host_state(&start.state_path, &host).await {
        node.close().await;
        let _ = updates.send(HostUpdate::Failed(error));
        return;
    }
    let status = match host.status().await {
        Ok(status) => status,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let uri = match expose_host_uri(&host).await {
        Ok(uri) => uri,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error));
            return;
        }
    };
    let mut uri_generation = status.uri_generation;
    if updates.send(HostUpdate::Started { uri, status }).is_err() {
        node.close().await;
        return;
    }

    let first_health_check = tokio::time::Instant::now() + MC_HEALTH_INTERVAL;
    let mut health_checks = tokio::time::interval_at(first_health_check, MC_HEALTH_INTERVAL);
    health_checks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut health_failures = 0_u8;
    let mut pending_target_error = None;
    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(HostCommand::Rotate) => {
                        if let Err(error) = host.rotate_token().await {
                            let _ = updates.send(HostUpdate::Error(error.to_string()));
                        }
                    }
                    Some(HostCommand::Stop { done }) => {
                        let result = host.stop().await.map_err(|error| error.to_string());
                        node.close().await;
                        let _ = updates.send(HostUpdate::Stopped(result));
                        if let Some(done) = done {
                            let _ = done.send(());
                        }
                        return;
                    }
                    None => {
                        node.close().await;
                        return;
                    }
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if is_target_unavailable_event(&event) {
                            pending_target_error.get_or_insert(event);
                        } else {
                            let _ = updates.send(HostUpdate::Event(event));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        let _ = updates.send(HostUpdate::Error(
                            format!("missed {count} host events")
                        ));
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        node.close().await;
                        let _ = updates.send(HostUpdate::Failed(
                            "host event channel closed unexpectedly".to_owned()
                        ));
                        return;
                    }
                }
            }
            status = statuses.recv() => {
                let Some(status) = status else {
                    node.close().await;
                    let _ = updates.send(HostUpdate::Failed(
                        "host status channel closed unexpectedly".to_owned()
                    ));
                    return;
                };
                if status.uri_generation > uri_generation {
                    uri_generation = status.uri_generation;
                    if let Err(error) = persist_host_state(&start.state_path, &host).await {
                        node.close().await;
                        let _ = updates.send(HostUpdate::Failed(error));
                        return;
                    }
                    match expose_host_uri(&host).await {
                        Ok(uri) => {
                            let _ = updates.send(HostUpdate::UriChanged { uri, status });
                        }
                        Err(error) => {
                            node.close().await;
                            let _ = updates.send(HostUpdate::Failed(error));
                            return;
                        }
                    }
                } else {
                    let _ = updates.send(HostUpdate::Status(status));
                }
            }
            _ = health_checks.tick() => {
                let available = minecraft_available(target_addr).await;
                if available
                    && let Some(event) = pending_target_error.take()
                {
                    let _ = updates.send(HostUpdate::Event(event));
                }
                if !record_health_check(&mut health_failures, available) {
                    continue;
                }

                let stop_result = host.stop().await.map_err(|error| error.to_string());
                node.close().await;
                let update = match stop_result {
                    Ok(()) => HostUpdate::MinecraftUnavailable,
                    Err(error) => HostUpdate::Failed(error),
                };
                let _ = updates.send(update);
                return;
            }
        }
    }
}

async fn minecraft_available(addr: SocketAddr) -> bool {
    tokio::task::spawn_blocking(move || probe_server(addr, MC_PROBE_TIMEOUT))
        .await
        .is_ok_and(|result| result.is_ok())
}

fn record_health_check(failures: &mut u8, available: bool) -> bool {
    if available {
        *failures = 0;
        return false;
    }
    *failures = failures.saturating_add(1);
    *failures >= MC_HEALTH_FAILURES_MAX
}

fn is_target_unavailable_event(event: &TunnelEvent) -> bool {
    matches!(
        event,
        TunnelEvent::Error {
            category: ErrorCategory::TargetUnavailable,
            ..
        }
    )
}

async fn persist_host_state(path: &Path, host: &HostedServiceHandle) -> Result<(), String> {
    let token_state = host
        .token_state()
        .await
        .map_err(|error| error.to_string())?;
    persist::save_host_state(
        path,
        &HostState {
            service_id: host.service_id(),
            token_state,
        },
    )
    .map_err(|error| error.to_string())
}

async fn expose_host_uri(host: &HostedServiceHandle) -> Result<String, String> {
    host.join_uri()
        .await
        .map_err(|error| error.to_string())?
        .expose_secret_uri()
        .map_err(|error| error.to_string())
}

fn spawn_join_subscription(
    rt: &tokio::runtime::Runtime,
    mut subscription: sculk::tunnel::TunnelSubscription,
    repaint: egui::Context,
) -> mpsc::UnboundedReceiver<TunnelUpdate> {
    let (tx, rx) = mpsc::unbounded_channel();
    rt.spawn(async move {
        while let Some(update) = subscription.recv().await {
            if tx.send(update).is_err() {
                break;
            }
            repaint.request_repaint();
        }
    });
    rx
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

pub(crate) fn token_refresh_policy(setting: TokenRefreshSetting) -> TokenRefreshPolicy {
    match setting {
        TokenRefreshSetting::Always => TokenRefreshPolicy::Always,
        TokenRefreshSetting::Never => TokenRefreshPolicy::Never,
        TokenRefreshSetting::OneHour => TokenRefreshPolicy::After(Duration::from_secs(60 * 60)),
        TokenRefreshSetting::ThreeHours => {
            TokenRefreshPolicy::After(Duration::from_secs(3 * 60 * 60))
        }
        TokenRefreshSetting::SixHours => {
            TokenRefreshPolicy::After(Duration::from_secs(6 * 60 * 60))
        }
        TokenRefreshSetting::TwelveHours => {
            TokenRefreshPolicy::After(Duration::from_secs(12 * 60 * 60))
        }
        TokenRefreshSetting::TwentyFourHours => {
            TokenRefreshPolicy::After(Duration::from_secs(24 * 60 * 60))
        }
    }
}

/// 将隧道事件格式化为人类可读的日志行。
fn format_event(event: &TunnelEvent) -> String {
    match event {
        TunnelEvent::PlayerJoined { id } => t!("player_joined", id = id).to_string(),
        TunnelEvent::PlayerLeft { id, reason } => {
            t!("player_left", id = id, reason = reason).to_string()
        }
        TunnelEvent::Connected => t!("connected_host").to_string(),
        TunnelEvent::Disconnected { reason } => t!("disconnected", reason = reason).to_string(),
        TunnelEvent::PathChanged {
            remote_id,
            is_relay,
            rtt_ms,
        } => {
            let route = if *is_relay {
                t!("relay_route")
            } else {
                t!("direct_route")
            };
            t!("path_changed", id = remote_id, route = route, rtt = rtt_ms).to_string()
        }
        TunnelEvent::Reconnecting { attempt } => t!("reconnecting", n = attempt).to_string(),
        TunnelEvent::Reconnected => t!("reconnected").to_string(),
        TunnelEvent::TokenRotated => t!("token_rotated").to_string(),
        TunnelEvent::TokenRotationFailed { retry_in } => {
            t!("token_rotation_failed", secs = retry_in.as_secs()).to_string()
        }
        TunnelEvent::AuthFailed { id } => t!("auth_failed", id = id).to_string(),
        TunnelEvent::PlayerRejected { id, reason } => {
            t!("rejected", id = id, reason = reason).to_string()
        }
        TunnelEvent::Error { message, .. } => t!("error_msg", msg = message).to_string(),
        other => format!("[?] {other:?}"),
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

    fn on_exit(&mut self) {
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
    fn maps_timed_refresh_policy() {
        assert_eq!(
            token_refresh_policy(TokenRefreshSetting::ThreeHours),
            TokenRefreshPolicy::After(Duration::from_secs(3 * 60 * 60))
        );
    }

    #[test]
    fn requires_three_consecutive_minecraft_probe_failures() {
        let mut failures = 0;
        assert!(!record_health_check(&mut failures, false));
        assert!(!record_health_check(&mut failures, false));
        assert!(record_health_check(&mut failures, false));
    }

    #[test]
    fn successful_minecraft_probe_resets_failures() {
        let mut failures = 2;
        assert!(!record_health_check(&mut failures, true));
        assert_eq!(failures, 0);
        assert!(!record_health_check(&mut failures, false));
    }

    #[test]
    fn identifies_target_unavailable_host_errors_only() {
        let target_unavailable = TunnelEvent::Error {
            category: ErrorCategory::TargetUnavailable,
            message: "target unavailable".to_owned(),
        };
        let internal = TunnelEvent::Error {
            category: ErrorCategory::Internal,
            message: "internal error".to_owned(),
        };

        assert!(is_target_unavailable_event(&target_unavailable));
        assert!(!is_target_unavailable_event(&internal));
        assert!(!is_target_unavailable_event(&TunnelEvent::Connected));
    }
}
