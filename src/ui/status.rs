use crate::app::App;
use eframe::egui;

use super::{BLUE, DIM, GREEN};

/// 渲染连接状态面板：显示活跃连接的 Peer、RTT、路由类型和流量。
pub fn render_status(app: &mut App, ui: &mut egui::Ui) {
    if !app.running || app.connections.is_empty() {
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

            for conn in &app.connections {
                let peer = format!("{:.8}", conn.remote_id.to_string());
                ui.label(&peer);
                ui.label(format!("{}ms", conn.rtt_ms));
                let route = if conn.is_relay {
                    t!("relay_route")
                } else {
                    t!("direct_route")
                };
                let color = if conn.is_relay { DIM } else { GREEN };
                ui.label(egui::RichText::new(route.as_ref()).color(color));
                ui.label(format_bytes(conn.tx_bytes + conn.rx_bytes));
                ui.end_row();
            }
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
