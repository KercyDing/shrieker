#![windows_subsystem = "windows"]

use eframe::egui;
use sculk::persist::{self, Profile};
use sculk::tunnel::{HostConfig, IrohTunnel, JoinConfig, SecretKey, Ticket, TunnelEvent};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes)
        .expect("failed to load icon")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let _guard = rt.enter();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 480.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "sculk demo",
        options,
        Box::new(|cc| {
            let ctx = &cc.egui_ctx;
            let mut style = (*ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(12.0, 4.0);
            let r = egui::CornerRadius::same(4);
            style.visuals.widgets.inactive.corner_radius = r;
            style.visuals.widgets.hovered.corner_radius = r;
            style.visuals.widgets.active.corner_radius = r;
            ctx.set_style(style);
            Ok(Box::new(App::new(rt)))
        }),
    )
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Host,
    Join,
}

enum UiMsg {
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

struct App {
    rt: tokio::runtime::Runtime,
    mode: Mode,
    host_port: String,
    password: String,
    max_players: String,
    ticket_input: String,
    join_port: String,
    join_password: String,
    tunnel: Option<Arc<IrohTunnel>>,
    ticket_display: Option<String>,
    logs: Vec<String>,
    event_rx: Option<mpsc::Receiver<TunnelEvent>>,
    ui_rx: mpsc::UnboundedReceiver<UiMsg>,
    ui_tx: mpsc::UnboundedSender<UiMsg>,
    running: bool,
    // Persist
    profile: Profile,
    _key_path: PathBuf,
    secret_key: Option<SecretKey>,
}

impl App {
    fn new(rt: tokio::runtime::Runtime) -> Self {
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();

        let (profile, profile_err) = match Profile::load() {
            Ok(p) => (p, None),
            Err(e) => (Profile::default(), Some(format!("[-] Profile load: {e}"))),
        };
        let key_path = persist::default_key_path().unwrap_or_else(|_| PathBuf::from("secret.key"));
        let (secret_key, key_err) = match persist::load_or_generate_key(&key_path) {
            Ok(k) => (Some(k), None),
            Err(e) => (None, Some(format!("[-] Key load: {e}"))),
        };

        let mut logs = Vec::new();
        if let Some(e) = profile_err {
            logs.push(e);
        }
        if let Some(e) = key_err {
            logs.push(e);
        }
        if logs.is_empty() {
            logs.push("[+] Profile and key loaded".into());
        }

        let host_port = profile.host.port.to_string();
        let join_port = profile.join.port.to_string();
        let ticket_input = profile.join.last_ticket.clone().unwrap_or_default();

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
            profile,
            _key_path: key_path,
            secret_key,
        }
    }

    /// Sync UI fields back to profile and save.
    fn save_profile(&mut self) {
        self.profile.host.port = self.host_port.parse().unwrap_or(25565);
        self.profile.join.port = self.join_port.parse().unwrap_or(30000);
        if !self.ticket_input.is_empty() {
            self.profile.join.last_ticket = Some(self.ticket_input.clone());
        }
        if let Err(e) = self.profile.save() {
            self.logs.push(format!("[-] Save profile: {e}"));
        }
    }

    fn start_host(&mut self) {
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
        let tx = self.ui_tx.clone();
        let secret_key = self.secret_key.clone();
        let relay_url = self.profile.resolve_relay_url(None).ok().unwrap_or(None);

        self.running = true;
        self.logs.push("[*] Starting tunnel...".into());
        self.save_profile();

        self.rt.spawn(async move {
            match IrohTunnel::host(port, secret_key, relay_url, config).await {
                Ok((tunnel, ticket, events)) => {
                    let ticket_str = ticket.to_string();
                    let _ = tx.send(UiMsg::Log("[+] Host ready".into()));
                    let _ = tx.send(UiMsg::HostReady {
                        tunnel: Arc::new(tunnel),
                        ticket: ticket_str,
                        events,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiMsg::Log(format!("[-] Host failed: {e}")));
                }
            }
        });
    }

    fn start_join(&mut self) {
        let ticket_str = self.ticket_input.clone();
        let port: u16 = self.join_port.parse().unwrap_or(30000);
        let password = if self.join_password.is_empty() {
            None
        } else {
            Some(self.join_password.clone())
        };
        let config = JoinConfig::default()
            .password(password);
        let tx = self.ui_tx.clone();

        self.running = true;
        self.logs.push("[*] Joining tunnel...".into());
        self.save_profile();

        self.rt.spawn(async move {
            let ticket = match ticket_str.parse::<Ticket>() {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx.send(UiMsg::Log(format!("[-] Invalid ticket: {e}")));
                    return;
                }
            };
            match IrohTunnel::join(&ticket, port, config).await {
                Ok((tunnel, events)) => {
                    let _ = tx.send(UiMsg::Log("[+] Joined!".into()));
                    let _ = tx.send(UiMsg::JoinReady {
                        tunnel: Arc::new(tunnel),
                        events,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiMsg::Log(format!("[-] Join failed: {e}")));
                }
            }
        });
    }

    fn stop(&mut self) {
        if let Some(tunnel) = self.tunnel.take() {
            let tx = self.ui_tx.clone();
            self.rt.spawn(async move {
                tunnel.close().await;
                let _ = tx.send(UiMsg::Log("[*] Tunnel closed".into()));
            });
        }
        self.running = false;
        self.ticket_display = None;
        self.event_rx = None;
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
                self.logs.push(format!("    {event:?}"));
            }
        }
    }
}

const GREEN: egui::Color32 = egui::Color32::from_rgb(74, 222, 128);
const BLUE: egui::Color32 = egui::Color32::from_rgb(125, 211, 252);
const DIM: egui::Color32 = egui::Color32::from_rgb(120, 120, 120);

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("sculk")
                        .strong()
                        .size(18.0)
                        .color(GREEN),
                );
                ui.label(egui::RichText::new("demo").size(18.0).color(DIM));
                ui.add_space(16.0);

                let enabled = !self.running;
                ui.add_enabled_ui(enabled, |ui| {
                    if ui
                        .selectable_label(self.mode == Mode::Host, egui::RichText::new("Host"))
                        .clicked()
                    {
                        self.mode = Mode::Host;
                    }
                    if ui
                        .selectable_label(self.mode == Mode::Join, egui::RichText::new("Join"))
                        .clicked()
                    {
                        self.mode = Mode::Join;
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = if self.running { "Connected" } else { "Idle" };
                    let color = if self.running { GREEN } else { DIM };
                    ui.label(egui::RichText::new(status).color(color).small());
                });
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.mode {
                Mode::Host => self.render_host(ui, ctx),
                Mode::Join => self.render_join(ui),
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Logs").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Clear").clicked() {
                        self.logs.clear();
                    }
                });
            });

            ui.add_space(2.0);
            let log_area = ui.available_rect_before_wrap();
            egui::Frame::new()
                .fill(ui.style().visuals.extreme_bg_color)
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(
                        log_area.width() - 16.0,
                        log_area.height() - 40.0,
                    ));
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.logs {
                                let color = if line.starts_with("[+]") {
                                    GREEN
                                } else if line.starts_with("[-]") {
                                    egui::Color32::from_rgb(248, 113, 113)
                                } else if line.starts_with("[*]") {
                                    BLUE
                                } else {
                                    DIM
                                };
                                ui.label(
                                    egui::RichText::new(line)
                                        .color(color)
                                        .family(egui::FontFamily::Monospace)
                                        .size(12.0),
                                );
                            }
                        });
                });
        });
    }
}

impl App {
    fn render_host(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Grid::new("host_cfg")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("MC Port");
                ui.add(egui::TextEdit::singleline(&mut self.host_port).desired_width(120.0));
                ui.end_row();
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(true)
                        .desired_width(120.0),
                );
                ui.end_row();
                ui.label("Max Players");
                ui.add(egui::TextEdit::singleline(&mut self.max_players).desired_width(120.0));
                ui.end_row();
            });

        ui.add_space(6.0);
        if self.running {
            if ui.button("Stop").clicked() {
                self.stop();
            }
        } else if ui.button("Start Host").clicked() {
            self.start_host();
        }

        if let Some(ticket) = &self.ticket_display {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Ticket").strong());
            let mut t = ticket.clone();
            ui.add(
                egui::TextEdit::singleline(&mut t)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            if ui.button("Copy to Clipboard").clicked() {
                ctx.copy_text(ticket.clone());
                self.logs.push("[+] Ticket copied".into());
            }
        }
    }

    fn render_join(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("join_cfg")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Ticket");
                ui.add(
                    egui::TextEdit::singleline(&mut self.ticket_input)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );
                ui.end_row();
                ui.label("Local Port");
                ui.add(egui::TextEdit::singleline(&mut self.join_port).desired_width(120.0));
                ui.end_row();
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.join_password)
                        .password(true)
                        .desired_width(120.0),
                );
                ui.end_row();
            });

        ui.add_space(6.0);
        if self.running {
            if ui.button("Disconnect").clicked() {
                self.stop();
            }
        } else if ui.button("Join").clicked() {
            self.start_join();
        }
    }
}
