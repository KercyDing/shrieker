use crate::settings;
use crate::ui;
use eframe::egui;
use sculk::persist::Profile;
use sculk::tunnel::{
    HostConfig, HostOptions, JoinConfig, JoinOptions, SecretKey, Ticket, TunnelEvent, TunnelMode,
    TunnelPhase, TunnelService, TunnelStatus, TunnelUpdate,
};
use tokio::sync::mpsc;

const UPDATES_PER_FRAME_MAX: usize = 256;

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Mode {
    Host,
    Join,
    Relay,
}

pub struct App {
    rt: tokio::runtime::Runtime,
    service: TunnelService,
    repaint: egui::Context,
    tunnel_rx: mpsc::UnboundedReceiver<TunnelUpdate>,
    stop_tx: mpsc::UnboundedSender<Result<(), String>>,
    stop_rx: mpsc::UnboundedReceiver<Result<(), String>>,
    stop_pending: bool,
    pub(crate) mode: Mode,
    pub(crate) host_port: String,
    pub(crate) password: String,
    pub(crate) max_players: String,
    pub(crate) ticket_input: String,
    pub(crate) join_port: String,
    pub(crate) join_password: String,
    pub(crate) logs: Vec<String>,
    pub(crate) profile: Profile,
    pub(crate) secret_key: Option<SecretKey>,
    pub(crate) relay_custom: bool,
    pub(crate) relay_url: String,
    pub(crate) tunnel: TunnelStatus,
    pub(crate) dark_mode: bool,
}

impl App {
    pub fn new(rt: tokio::runtime::Runtime, repaint: egui::Context) -> Self {
        let loaded = settings::load();
        rust_i18n::set_locale(&loaded.locale);

        let service = TunnelService::new();
        let tunnel = service.status();
        let tunnel_rx = spawn_subscription(&rt, service.subscribe(), repaint.clone());
        let (stop_tx, stop_rx) = mpsc::unbounded_channel();

        let mut logs = loaded.errors;
        if logs.is_empty() {
            logs.push(t!("profile_loaded").to_string());
        }

        Self {
            rt,
            service,
            repaint,
            tunnel_rx,
            stop_tx,
            stop_rx,
            stop_pending: false,
            mode: Mode::Host,
            host_port: loaded.profile.host.port.to_string(),
            password: String::new(),
            max_players: String::new(),
            ticket_input: loaded.profile.join.last_ticket.clone().unwrap_or_default(),
            join_port: loaded.profile.join.port.to_string(),
            join_password: String::new(),
            relay_custom: loaded.profile.relay.custom,
            relay_url: loaded.profile.relay.url.clone().unwrap_or_default(),
            profile: loaded.profile,
            secret_key: loaded.secret_key,
            logs,
            tunnel,
            dark_mode: loaded.dark_mode,
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.tunnel.state.phase == TunnelPhase::Idle
    }

    pub(crate) fn stop_pending(&self) -> bool {
        self.stop_pending
    }

    /// 将界面字段同步到 profile 并保存。
    pub(crate) fn save_profile(&mut self) {
        if let Ok(port) = self.host_port.parse() {
            self.profile.host.port = port;
        }
        if let Ok(port) = self.join_port.parse() {
            self.profile.join.port = port;
        }
        if !self.ticket_input.is_empty() {
            self.profile.join.last_ticket = Some(self.ticket_input.clone());
        }
        self.profile.relay.custom = self.relay_custom;
        self.profile.relay.url = (!self.relay_url.is_empty()).then(|| self.relay_url.clone());
        if let Err(error) = self.profile.save() {
            self.logs
                .push(t!("save_profile_err", err = error).to_string());
        }
    }

    pub(crate) fn start_host(&mut self) {
        let port = match self.host_port.parse::<u16>() {
            Ok(port) => port,
            Err(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                return;
            }
        };
        let max_players = match parse_optional_u32(&self.max_players) {
            Ok(value) => value,
            Err(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                return;
            }
        };
        let relay_url = match self.profile.resolve_relay_url(None) {
            Ok(relay_url) => relay_url,
            Err(error) => {
                self.logs.push(t!("host_failed", err = error).to_string());
                return;
            }
        };
        let config = HostConfig::default()
            .password(non_empty(&self.password))
            .max_players(max_players);
        let options = HostOptions::new(port)
            .secret_key(self.secret_key.clone())
            .relay_url(relay_url)
            .config(config);

        self.save_profile();
        self.logs.push(t!("starting_tunnel").to_string());
        if let Err(error) = self.rt.block_on(self.service.start_host(options)) {
            self.logs.push(t!("host_failed", err = error).to_string());
        }
        self.tunnel = self.service.status();
    }

    pub(crate) fn start_join(&mut self) {
        let ticket = match self.ticket_input.trim().parse::<Ticket>() {
            Ok(ticket) => ticket,
            Err(error) => {
                self.logs
                    .push(t!("invalid_ticket", err = error).to_string());
                return;
            }
        };
        let port = match self.join_port.parse::<u16>() {
            Ok(port) => port,
            Err(error) => {
                self.logs.push(t!("join_failed", err = error).to_string());
                return;
            }
        };
        let config = JoinConfig::default().password(non_empty(&self.join_password));
        let options = JoinOptions::new(ticket, port).config(config);

        self.save_profile();
        self.logs.push(t!("joining_tunnel").to_string());
        if let Err(error) = self.rt.block_on(self.service.start_join(options)) {
            self.logs.push(t!("join_failed", err = error).to_string());
        }
        self.tunnel = self.service.status();
    }

    pub(crate) fn stop(&mut self) {
        if self.stop_pending {
            return;
        }
        self.stop_pending = true;

        let service = self.service.clone();
        let tx = self.stop_tx.clone();
        let repaint = self.repaint.clone();
        self.rt.spawn(async move {
            let result = service.shutdown().await.map_err(|error| error.to_string());
            let _ = tx.send(result);
            repaint.request_repaint();
        });
    }

    fn poll(&mut self) {
        for _ in 0..UPDATES_PER_FRAME_MAX {
            let Ok(update) = self.tunnel_rx.try_recv() else {
                break;
            };
            self.apply_tunnel_update(update);
        }
        if let Ok(result) = self.stop_rx.try_recv() {
            self.stop_pending = false;
            if let Err(error) = result {
                self.logs.push(t!("stop_failed", err = error).to_string());
            }
        }
    }

    fn apply_tunnel_update(&mut self, update: TunnelUpdate) {
        match update {
            TunnelUpdate::Status(status) => self.apply_status(status),
            TunnelUpdate::Event(event) => self.logs.push(format_event(&event)),
            _ => {}
        }
    }

    fn apply_status(&mut self, status: TunnelStatus) {
        let previous = self.tunnel.state.clone();
        let current = &status.state;

        if previous.phase != TunnelPhase::Active && current.phase == TunnelPhase::Active {
            match current.mode {
                Some(TunnelMode::Host) => {
                    self.logs.push(t!("host_ready").to_string());
                    if let Some(ticket) = &current.ticket
                        && sculk::clipboard::clipboard_copy(&ticket.to_string())
                    {
                        self.logs.push(t!("ticket_copied").to_string());
                    }
                }
                Some(TunnelMode::Join) => self.logs.push(t!("joined").to_string()),
                None => {}
            }
        } else if previous.phase == TunnelPhase::Stopping && current.phase == TunnelPhase::Idle {
            self.logs.push(t!("tunnel_closed").to_string());
        }

        self.tunnel = status;
    }

    /// 切换语言。
    pub(crate) fn toggle_lang(&mut self) {
        let current = rust_i18n::locale();
        if &*current == "zh-CN" {
            rust_i18n::set_locale("en");
        } else {
            rust_i18n::set_locale("zh-CN");
        }
        self.save_preferences();
    }

    /// 切换主题。
    pub(crate) fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.dark_mode = !self.dark_mode;
        let theme = if self.dark_mode {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        self.save_preferences();
    }

    fn save_preferences(&mut self) {
        if let Err(error) = settings::save_preferences(self.dark_mode, rust_i18n::locale().as_ref())
        {
            self.logs
                .push(t!("save_preferences_err", err = error).to_string());
        }
    }
}

fn spawn_subscription(
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

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_optional_u32(value: &str) -> Result<Option<u32>, std::num::ParseIntError> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse().map(Some)
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
        TunnelEvent::AuthFailed { id } => t!("auth_failed", id = id).to_string(),
        TunnelEvent::PlayerRejected { id, reason } => {
            t!("rejected", id = id, reason = reason).to_string()
        }
        TunnelEvent::Error { message } => t!("error_msg", msg = message).to_string(),
        other => format!("[?] {other:?}"),
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();

        if self.stop_pending || !self.is_idle() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::render(self, root);
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
    }
}
