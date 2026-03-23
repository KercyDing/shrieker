use crate::services;
use crate::ui;
use eframe::egui;
use sculk::persist::Profile;
use sculk::tunnel::{
    ConnectionSnapshot, HostConfig, IrohTunnel, JoinConfig, SecretKey, TunnelEvent,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Mode {
    Host,
    Join,
    Relay,
}

pub enum UiMsg {
    Log(String),
    HostReady {
        tunnel: Arc<IrohTunnel>,
        ticket: String,
        events: mpsc::Receiver<TunnelEvent>,
    },
    JoinReady {
        tunnel: Arc<IrohTunnel>,
        events: mpsc::Receiver<TunnelEvent>,
    },
}

pub struct App {
    pub(crate) rt: tokio::runtime::Runtime,
    pub(crate) mode: Mode,
    pub(crate) host_port: String,
    pub(crate) password: String,
    pub(crate) max_players: String,
    pub(crate) ticket_input: String,
    pub(crate) join_port: String,
    pub(crate) join_password: String,
    pub(crate) tunnel: Option<Arc<IrohTunnel>>,
    pub(crate) ticket_display: Option<String>,
    pub(crate) logs: Vec<String>,
    pub(crate) event_rx: Option<mpsc::Receiver<TunnelEvent>>,
    pub(crate) ui_rx: mpsc::UnboundedReceiver<UiMsg>,
    pub(crate) ui_tx: mpsc::UnboundedSender<UiMsg>,
    pub(crate) running: bool,
    pub(crate) profile: Profile,
    pub(crate) _key_path: PathBuf,
    pub(crate) secret_key: Option<SecretKey>,
    pub(crate) relay_custom: bool,
    pub(crate) relay_url: String,
    pub(crate) connections: Vec<ConnectionSnapshot>,
    pub(crate) dark_mode: bool,
}

impl App {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();

        let ps = services::persist::load();
        let prefs = services::persist::load_prefs();
        rust_i18n::set_locale(&prefs.locale);

        let mut logs = ps.errors;
        if logs.is_empty() {
            logs.push(t!("profile_loaded").to_string());
        }

        let host_port = ps.profile.host.port.to_string();
        let join_port = ps.profile.join.port.to_string();
        let ticket_input = ps.profile.join.last_ticket.clone().unwrap_or_default();
        let relay_custom = ps.profile.relay.custom;
        let relay_url = ps.profile.relay.url.clone().unwrap_or_default();

        Self {
            rt,
            mode: Mode::Host,
            host_port,
            password: String::new(),
            max_players: String::new(),
            ticket_input,
            join_port,
            join_password: String::new(),
            tunnel: None,
            ticket_display: None,
            logs,
            event_rx: None,
            ui_rx,
            ui_tx,
            running: false,
            profile: ps.profile,
            _key_path: ps.key_path,
            secret_key: ps.secret_key,
            relay_custom,
            relay_url,
            connections: Vec::new(),
            dark_mode: prefs.dark_mode,
        }
    }

    /// 将 UI 字段同步回 profile 并保存。
    pub(crate) fn save_profile(&mut self) {
        self.profile.host.port = self.host_port.parse().unwrap_or(25565);
        self.profile.join.port = self.join_port.parse().unwrap_or(30000);
        if !self.ticket_input.is_empty() {
            self.profile.join.last_ticket = Some(self.ticket_input.clone());
        }
        self.profile.relay.custom = self.relay_custom;
        self.profile.relay.url = if self.relay_url.is_empty() {
            None
        } else {
            Some(self.relay_url.clone())
        };
        if let Err(e) = self.profile.save() {
            self.logs.push(t!("save_profile_err", err = e).to_string());
        }
    }

    pub(crate) fn start_host(&mut self) {
        let port: u16 = self.host_port.parse().unwrap_or(25565);
        let password = if self.password.is_empty() {
            None
        } else {
            Some(self.password.clone())
        };
        let max_players: Option<u32> = self.max_players.parse().ok();
        let config = HostConfig::default()
            .password(password)
            .max_players(max_players);
        let secret_key = self.secret_key.clone();
        let relay_url = self.profile.resolve_relay_url(None).ok().unwrap_or(None);

        self.running = true;
        self.logs.push(t!("starting_tunnel").to_string());
        self.save_profile();

        services::tunnel::spawn_host(
            &self.rt,
            self.ui_tx.clone(),
            port,
            secret_key,
            relay_url,
            config,
        );
    }

    pub(crate) fn start_join(&mut self) {
        let ticket_str = self.ticket_input.clone();
        let port: u16 = self.join_port.parse().unwrap_or(30000);
        let password = if self.join_password.is_empty() {
            None
        } else {
            Some(self.join_password.clone())
        };
        let config = JoinConfig::default().password(password);

        self.running = true;
        self.logs.push(t!("joining_tunnel").to_string());
        self.save_profile();

        services::tunnel::spawn_join(&self.rt, self.ui_tx.clone(), ticket_str, port, config);
    }

    pub(crate) fn stop(&mut self) {
        if let Some(tunnel) = self.tunnel.take() {
            services::tunnel::spawn_close(&self.rt, self.ui_tx.clone(), tunnel);
        }
        self.running = false;
        self.ticket_display = None;
        self.event_rx = None;
        self.connections.clear();
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                UiMsg::Log(s) => self.logs.push(s),
                UiMsg::HostReady {
                    tunnel,
                    ticket,
                    events,
                } => {
                    if sculk::clipboard::clipboard_copy(&ticket) {
                        self.logs.push(t!("ticket_copied").to_string());
                    }
                    self.tunnel = Some(tunnel);
                    self.ticket_display = Some(ticket);
                    self.event_rx = Some(events);
                }
                UiMsg::JoinReady { tunnel, events } => {
                    self.tunnel = Some(tunnel);
                    self.event_rx = Some(events);
                }
            }
        }
        if let Some(rx) = &mut self.event_rx {
            while let Ok(event) = rx.try_recv() {
                self.logs.push(format_event(&event));
            }
        }
        if let Some(tunnel) = &self.tunnel
            && let Ok(conns) = tunnel.connections()
        {
            self.connections = conns;
        }
    }

    /// 切换语言。
    pub(crate) fn toggle_lang(&mut self) {
        let current = rust_i18n::locale();
        if &*current == "zh-CN" {
            rust_i18n::set_locale("en");
        } else {
            rust_i18n::set_locale("zh-CN");
        }
        services::persist::save_prefs(self.dark_mode, &rust_i18n::locale());
    }

    /// 切换主题。
    pub(crate) fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.dark_mode = !self.dark_mode;
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
        services::persist::save_prefs(self.dark_mode, &rust_i18n::locale());
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();

        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        ui::render_header(self, ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.mode {
                Mode::Host => ui::render_host(self, ui, ctx),
                Mode::Join => ui::render_join(self, ui),
                Mode::Relay => ui::render_relay(self, ui),
            }

            ui::render_status(self, ui);
            ui::render_logs(self, ui);
        });
    }
}
