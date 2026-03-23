use crate::app::App;
use eframe::egui;

use super::{BLUE, DIM, GREEN, RED};

/// 渲染日志面板。
pub fn render_logs(app: &mut App, ui: &mut egui::Ui) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(2.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(t!("logs").as_ref()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(t!("clear").as_ref()).clicked() {
                app.logs.clear();
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
                                .family(egui::FontFamily::Monospace)
                                .size(12.0),
                        );
                    }
                });
        });
}
