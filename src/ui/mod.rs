use eframe::egui;

pub const GREEN: egui::Color32 = egui::Color32::from_rgb(74, 222, 128);
pub const BLUE: egui::Color32 = egui::Color32::from_rgb(125, 211, 252);
pub const RED: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);
pub const DIM: egui::Color32 = egui::Color32::from_rgb(120, 120, 120);

mod header;
mod host;
mod join;
mod logs;
mod relay;
mod status;

pub use header::render_header;
pub use host::render_host;
pub use join::render_join;
pub use logs::render_logs;
pub use relay::render_relay;
pub use status::render_status;
