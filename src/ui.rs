use crate::app::{App, Mode};
use crate::settings::CloseAction;
use eframe::egui;
use sculk::persist::TokenRefreshSetting;
use sculk::tunnel::TunnelPhase;
use std::time::{Duration, SystemTime};

const GREEN: egui::Color32 = egui::Color32::from_rgb(74, 222, 128);
const BLUE: egui::Color32 = egui::Color32::from_rgb(125, 211, 252);
const YELLOW: egui::Color32 = egui::Color32::from_rgb(250, 204, 21);
const RED: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);
const DIM: egui::Color32 = egui::Color32::from_rgb(120, 120, 120);
const HOST_FIELD_WIDTH: f32 = 130.0;

/// 渲染完整应用界面。
pub fn render(app: &mut App, root: &mut egui::Ui) {
    let ctx = root.ctx().clone();
    render_header(app, root);

    egui::CentralPanel::default().show(root, |ui| {
        match app.mode {
            Mode::Host => render_host(app, ui, &ctx),
            Mode::Join => render_join(app, ui),
            Mode::Settings => render_settings(app, ui),
            Mode::Preferences => render_preferences(app, ui, &ctx),
        }

        if matches!(app.mode, Mode::Host | Mode::Join) {
            render_status(app, ui);
            render_logs(app, ui);
        }
    });
}

fn render_header(app: &mut App, root: &mut egui::Ui) {
    egui::Panel::top("header").show(root, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("shrieker")
                    .strong()
                    .size(18.0)
                    .color(GREEN),
            );
            ui.add_space(16.0);

            let enabled = app.is_idle() && !app.stop_pending();
            ui.add_enabled_ui(enabled, |ui| {
                if ui
                    .selectable_label(app.mode == Mode::Host, t!("host"))
                    .clicked()
                {
                    app.set_mode(Mode::Host);
                }
                if ui
                    .selectable_label(app.mode == Mode::Join, t!("join"))
                    .clicked()
                {
                    app.set_mode(Mode::Join);
                }
                if ui
                    .selectable_label(app.mode == Mode::Settings, t!("settings"))
                    .clicked()
                {
                    app.set_mode(Mode::Settings);
                }
                if ui
                    .selectable_label(app.mode == Mode::Preferences, t!("preferences"))
                    .clicked()
                {
                    app.set_mode(Mode::Preferences);
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = match app.phase() {
                    TunnelPhase::Idle => t!("idle"),
                    TunnelPhase::Starting => t!("starting"),
                    TunnelPhase::Active => t!("connected"),
                    TunnelPhase::Stopping => t!("stopping"),
                };
                let color = if app.phase() == TunnelPhase::Active {
                    GREEN
                } else {
                    DIM
                };
                ui.label(egui::RichText::new(status.as_ref()).color(color).small());
            });
        });
        ui.add_space(4.0);
    });
}

fn render_host(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.add_enabled_ui(app.is_idle(), |ui| {
        egui::Grid::new("host_cfg")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(t!("port_mode").as_ref());
                egui::ComboBox::from_id_salt("host_port_mode")
                    .width(HOST_FIELD_WIDTH)
                    .selected_text(if app.host_manual_port {
                        t!("port_manual")
                    } else {
                        t!("port_auto")
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.host_manual_port,
                            false,
                            t!("port_auto").as_ref(),
                        );
                        ui.selectable_value(
                            &mut app.host_manual_port,
                            true,
                            t!("port_manual").as_ref(),
                        );
                    });
                ui.end_row();
                ui.label(t!("mc_port").as_ref());
                if app.host_manual_port {
                    let response = ui.add_sized(
                        [HOST_FIELD_WIDTH, ui.spacing().interact_size.y],
                        egui::TextEdit::singleline(&mut app.host_port),
                    );
                    if response.changed() {
                        app.save_host_port();
                    }
                } else {
                    match app.detected_mc_port {
                        Some(port) => {
                            ui.label(
                                egui::RichText::new(t!("mc_server_detected", port = port))
                                    .color(GREEN),
                            );
                        }
                        None => {
                            ui.label(egui::RichText::new(t!("mc_server_scanning")).color(YELLOW));
                        }
                    }
                }
                ui.end_row();
                ui.label(t!("max_players").as_ref());
                ui.add_sized(
                    [HOST_FIELD_WIDTH, ui.spacing().interact_size.y],
                    egui::TextEdit::singleline(&mut app.max_players),
                );
                ui.end_row();
                ui.label(t!("share_uri_lifetime").as_ref());
                let previous_token_refresh = app.token_refresh;
                egui::ComboBox::from_id_salt("token_refresh")
                    .width(HOST_FIELD_WIDTH)
                    .selected_text(token_refresh_label(app.token_refresh))
                    .show_ui(ui, |ui| {
                        for setting in token_refresh_settings() {
                            ui.selectable_value(
                                &mut app.token_refresh,
                                setting,
                                token_refresh_label(setting),
                            );
                        }
                    });
                if app.token_refresh != previous_token_refresh {
                    app.save_host_token_refresh();
                }
                ui.end_row();
            });
    });

    ui.add_space(6.0);
    if app.is_idle() {
        if action_button(
            ui,
            !app.stop_pending() && app.host_port_ready(),
            t!("start_host").as_ref(),
        ) {
            app.start_host();
        }
    } else if action_button(ui, !app.stop_pending(), t!("stop").as_ref()) {
        app.stop();
    }

    if let Some(share_uri) = app.share_uri.clone() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(t!("share_uri").as_ref()).strong());
        let mut display = share_uri.clone();
        ui.add(
            egui::TextEdit::singleline(&mut display)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button(t!("copy").as_ref()).clicked() {
                ctx.copy_text(share_uri.clone());
                app.logs.push(t!("share_uri_copied").to_string());
            }
            if action_button(
                ui,
                !app.rotate_pending() && !app.stop_pending(),
                t!("refresh_share_uri").as_ref(),
            ) {
                app.rotate_host_uri();
            }
        });
        if let Some(deadline) = app
            .host_status
            .as_ref()
            .and_then(|status| status.next_rotation_at)
        {
            ui.label(
                egui::RichText::new(t!(
                    "next_refresh",
                    time = format_remaining(deadline, SystemTime::now())
                ))
                .small()
                .color(DIM),
            );
        }
    }
}

fn render_join(app: &mut App, ui: &mut egui::Ui) {
    ui.add_enabled_ui(app.is_idle(), |ui| {
        egui::Grid::new("join_cfg")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(t!("share_uri").as_ref());
                let mut output = egui::TextEdit::singleline(&mut app.join_uri_input)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .show(ui);
                if app.join_uri_select_all {
                    use egui::text::{CCursor, CCursorRange};

                    output.response.request_focus();
                    let end = CCursor::new(app.join_uri_input.chars().count());
                    output
                        .state
                        .cursor
                        .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
                    output.state.store(ui.ctx(), output.response.id);
                    app.join_uri_select_all = false;
                    ui.ctx().request_repaint();
                }
                ui.end_row();
                ui.label(t!("local_port").as_ref());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.join_auto_port, t!("auto_port").as_ref());
                    let port_response = ui.add_enabled(
                        !app.join_auto_port,
                        egui::TextEdit::singleline(&mut app.join_port).desired_width(90.0),
                    );
                    if port_response.changed() {
                        app.save_join_port();
                    }
                });
                ui.end_row();
            });
    });

    ui.add_space(6.0);
    if app.is_idle() {
        if action_button(ui, !app.stop_pending(), t!("join_btn").as_ref()) {
            app.start_join();
        }
    } else if action_button(ui, !app.stop_pending(), t!("disconnect").as_ref()) {
        app.stop();
    }

    if let Some(addr) = app.join_local_addr() {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("minecraft_address").as_ref()).strong());
            ui.monospace(addr.to_string());
            if ui.small_button(t!("copy").as_ref()).clicked() {
                ui.ctx().copy_text(addr.to_string());
                app.logs.push(t!("address_copied").to_string());
            }
        });
    }
}

fn action_button(ui: &mut egui::Ui, enabled: bool, label: &str) -> bool {
    ui.add_enabled(enabled, egui::Button::new(label)).clicked()
}

fn render_settings(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(t!("relay_settings").as_ref()).strong());
    ui.add_space(2.0);
    ui.radio_value(&mut app.relay_custom, false, t!("default_relay").as_ref());
    ui.add_space(4.0);
    ui.radio_value(&mut app.relay_custom, true, t!("custom_relay").as_ref());

    if app.relay_custom {
        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut app.relay_url);
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new(t!("reconnect_settings").as_ref()).strong());
    ui.add_space(2.0);
    egui::Grid::new("reconnect_cfg")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label(t!("max_retries").as_ref());
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut app.reconnect_unlimited,
                    t!("unlimited_retries").as_ref(),
                );
                ui.add_enabled(
                    !app.reconnect_unlimited,
                    egui::DragValue::new(&mut app.reconnect_max_retries).range(0..=1000),
                );
            });
            ui.end_row();

            ui.label(t!("reconnect_interval").as_ref());
            ui.add(
                egui::DragValue::new(&mut app.reconnect_interval_secs)
                    .range(1..=300)
                    .suffix(" s"),
            );
            ui.end_row();
        });

    ui.add_space(8.0);
    if ui.button(t!("save").as_ref()).clicked() {
        app.save_settings();
    }
}

fn render_preferences(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.label(egui::RichText::new(t!("language").as_ref()).strong());
    ui.add_space(2.0);
    let current_locale = rust_i18n::locale();
    if ui.radio(&*current_locale == "zh-CN", "简体中文").clicked() {
        app.set_language("zh-CN");
    }
    ui.add_space(4.0);
    if ui.radio(&*current_locale == "en", "English").clicked() {
        app.set_language("en");
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(egui::RichText::new(t!("appearance").as_ref()).strong());
    ui.add_space(2.0);

    let mut theme = app.theme_preference;
    ui.horizontal(|ui| {
        ui.label(t!("theme").as_ref());
        ui.radio_value(
            &mut theme,
            egui::ThemePreference::System,
            t!("theme_system").as_ref(),
        );
        ui.radio_value(
            &mut theme,
            egui::ThemePreference::Light,
            t!("theme_light").as_ref(),
        );
        ui.radio_value(
            &mut theme,
            egui::ThemePreference::Dark,
            t!("theme_dark").as_ref(),
        );
    });
    if theme != app.theme_preference {
        app.set_theme(theme, ctx);
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(t!("close_action").as_ref());
        let mut close_action = app.close_action;
        ui.radio_value(
            &mut close_action,
            CloseAction::HideToTray,
            t!("close_hide_to_tray").as_ref(),
        );
        ui.radio_value(
            &mut close_action,
            CloseAction::Exit,
            t!("close_exit").as_ref(),
        );
        if close_action != app.close_action {
            app.set_close_action(close_action);
        }
    });

    ui.add_space(8.0);
    let mut remember_window_state = app.remember_window_state;
    if ui
        .checkbox(
            &mut remember_window_state,
            t!("remember_window_state").as_ref(),
        )
        .changed()
    {
        app.set_remember_window_state(remember_window_state);
    }

    ui.add_space(8.0);
    if ui.button(t!("save").as_ref()).clicked() {
        app.save_preference_settings();
    }
}

fn render_status(app: &App, ui: &mut egui::Ui) {
    if app.phase() == TunnelPhase::Idle {
        return;
    }

    if let Some(status) = &app.host_status {
        ui.separator();
        egui::Grid::new("host_status")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(t!("players").as_ref());
                ui.label(status.connection_count.to_string());
                ui.end_row();
                ui.label(t!("active_bridges").as_ref());
                ui.label(status.bridge_count.to_string());
                ui.end_row();
            });
        return;
    }

    let connections = app.join_connections();
    if connections.is_empty() {
        return;
    }

    ui.separator();
    ui.label(
        egui::RichText::new(t!("connections").as_ref())
            .color(BLUE)
            .strong(),
    );
    ui.add_space(2.0);

    egui::Grid::new("conn_grid")
        .num_columns(4)
        .spacing([16.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new(t!("peer").as_ref()).color(DIM));
            ui.label(egui::RichText::new(t!("rtt").as_ref()).color(DIM));
            ui.label(egui::RichText::new(t!("route").as_ref()).color(DIM));
            ui.label(egui::RichText::new(t!("traffic").as_ref()).color(DIM));
            ui.end_row();

            for connection in connections {
                let peer = format!("{:.8}", connection.remote_id.to_string());
                ui.label(peer);
                ui.label(format!("{}ms", connection.rtt_ms));
                let route = if connection.is_relay {
                    t!("relay_route")
                } else {
                    t!("direct_route")
                };
                let color = if connection.is_relay { DIM } else { GREEN };
                ui.label(egui::RichText::new(route.as_ref()).color(color));
                ui.label(format_bytes(connection.tx_bytes + connection.rx_bytes));
                ui.end_row();
            }
        });
}

fn token_refresh_settings() -> [TokenRefreshSetting; 7] {
    [
        TokenRefreshSetting::Always,
        TokenRefreshSetting::Never,
        TokenRefreshSetting::OneHour,
        TokenRefreshSetting::ThreeHours,
        TokenRefreshSetting::SixHours,
        TokenRefreshSetting::TwelveHours,
        TokenRefreshSetting::TwentyFourHours,
    ]
}

fn token_refresh_label(setting: TokenRefreshSetting) -> String {
    match setting {
        TokenRefreshSetting::Always => t!("lifetime_always").to_string(),
        TokenRefreshSetting::Never => t!("lifetime_never").to_string(),
        TokenRefreshSetting::OneHour => t!("lifetime_1h").to_string(),
        TokenRefreshSetting::ThreeHours => t!("lifetime_3h").to_string(),
        TokenRefreshSetting::SixHours => t!("lifetime_6h").to_string(),
        TokenRefreshSetting::TwelveHours => t!("lifetime_12h").to_string(),
        TokenRefreshSetting::TwentyFourHours => t!("lifetime_24h").to_string(),
    }
}

fn format_remaining(deadline: SystemTime, now: SystemTime) -> String {
    let remaining = deadline.duration_since(now).unwrap_or(Duration::ZERO);
    let seconds = remaining.as_secs();
    if seconds >= 60 * 60 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

fn render_logs(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(2.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(t!("logs").as_ref()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(t!("clear").as_ref()).clicked() {
                app.logs.clear();
            }
            if ui.small_button(t!("copy_logs").as_ref()).clicked() {
                let confirmations = [
                    t!("logs_copied", locale = "en").to_string(),
                    t!("logs_copied", locale = "zh-CN").to_string(),
                ];
                app.logs
                    .retain(|line| !confirmations.iter().any(|message| message == line));
                ui.ctx().copy_text(app.logs.join("\n"));
                app.logs.push(t!("logs_copied").to_string());
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
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &app.logs {
                        let color = if line.starts_with("[+]") {
                            GREEN
                        } else if line.starts_with("[-]") {
                            RED
                        } else if line.starts_with("[*]") {
                            BLUE
                        } else {
                            DIM
                        };
                        ui.label(
                            egui::RichText::new(line)
                                .color(color)
                                .family(egui::FontFamily::Monospace),
                        );
                    }
                });
        });
}

/// 将字节数格式化为人类可读的字符串。
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_remaining};
    use std::time::{Duration, SystemTime};

    #[test]
    fn formats_byte_boundaries() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn formats_remaining_time() {
        let now = SystemTime::UNIX_EPOCH;
        assert_eq!(format_remaining(now + Duration::from_secs(90), now), "1m");
        assert_eq!(
            format_remaining(now + Duration::from_secs(3 * 3600 + 120), now),
            "3h 2m"
        );
        assert_eq!(format_remaining(now, now + Duration::from_secs(1)), "0s");
    }
}
