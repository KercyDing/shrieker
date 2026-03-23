use crate::app::{App, Mode};
use eframe::egui;

use super::{DIM, GREEN};

/// 渲染顶部面板：标题 + 模式切换 + 连接状态。
pub fn render_header(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("header").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("shrieker")
                    .strong()
                    .size(18.0)
                    .color(GREEN),
            );
            ui.add_space(16.0);

            let enabled = !app.running;
            ui.add_enabled_ui(enabled, |ui| {
                if ui
                    .selectable_label(app.mode == Mode::Host, egui::RichText::new("Host"))
                    .clicked()
                {
                    app.mode = Mode::Host;
                }
                if ui
                    .selectable_label(app.mode == Mode::Join, egui::RichText::new("Join"))
                    .clicked()
                {
                    app.mode = Mode::Join;
                }
                if ui
                    .selectable_label(app.mode == Mode::Relay, egui::RichText::new("Relay"))
                    .clicked()
                {
                    app.mode = Mode::Relay;
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = if app.running { "Connected" } else { "Idle" };
                let color = if app.running { GREEN } else { DIM };
                ui.label(egui::RichText::new(status).color(color).small());
            });
        });
        ui.add_space(4.0);
    });
}
