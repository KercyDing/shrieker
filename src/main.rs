#![windows_subsystem = "windows"]

#[macro_use]
extern crate rust_i18n;

mod app;
mod services;
mod ui;

i18n!("locales", fallback = "en");

use eframe::egui;

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

fn setup_cjk_fonts(ctx: &egui::Context) {
    let candidates: &[&str] = &[
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
        // Linux
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    ];

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "cjk".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("cjk".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            ctx.set_fonts(fonts);
            return;
        }
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
        "shrieker",
        options,
        Box::new(|cc| {
            let ctx = &cc.egui_ctx;
            setup_cjk_fonts(ctx);
            ctx.all_styles_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 6.0);
                style.spacing.button_padding = egui::vec2(12.0, 4.0);
                let radius = egui::CornerRadius::same(4);
                style.visuals.widgets.inactive.corner_radius = radius;
                style.visuals.widgets.hovered.corner_radius = radius;
                style.visuals.widgets.active.corner_radius = radius;
            });
            let app = app::App::new(rt);
            let theme = if app.dark_mode {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            };
            ctx.set_theme(theme);
            Ok(Box::new(app))
        }),
    )
}
