#![windows_subsystem = "windows"]

#[macro_use]
extern crate rust_i18n;

mod app;
mod settings;
mod ui;

i18n!("locales", fallback = "en");

use eframe::egui;

const UI_FONT_SIZE: f32 = 14.0;

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

fn setup_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(font) = load_font(system_font_candidates()) {
        fonts.font_data.insert("system_ui".to_owned(), font.into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "system_ui".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

fn setup_ui_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 3.0);
        style.spacing.button_padding = egui::vec2(6.0, 2.0);
        style.spacing.indent = 14.0;
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(UI_FONT_SIZE + 3.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(UI_FONT_SIZE, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(UI_FONT_SIZE, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(UI_FONT_SIZE - 2.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(UI_FONT_SIZE, egui::FontFamily::Monospace),
        );
        let radius = egui::CornerRadius::same(4);
        style.visuals.widgets.inactive.corner_radius = radius;
        style.visuals.widgets.hovered.corner_radius = radius;
        style.visuals.widgets.active.corner_radius = radius;
    });
}

fn load_font(paths: Vec<std::path::PathBuf>) -> Option<egui::FontData> {
    for path in paths {
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        return Some(egui::FontData::from_owned(data));
    }
    None
}

#[cfg(target_os = "windows")]
fn system_font_candidates() -> Vec<std::path::PathBuf> {
    let font_dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("Fonts");
    [
        "msyh.ttc",
        "msyhl.ttc",
        "msyhbd.ttc",
        "simhei.ttf",
        "simsun.ttc",
    ]
    .into_iter()
    .map(|file_name| font_dir.join(file_name))
    .collect()
}

#[cfg(target_os = "macos")]
fn system_font_candidates() -> Vec<std::path::PathBuf> {
    path_candidates([
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_font_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = fontconfig_candidates(["sans:lang=zh-cn", "Noto Sans CJK SC"]);
    candidates.extend(path_candidates([
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    ]));
    candidates
}

#[cfg(not(any(unix, target_os = "windows")))]
fn system_font_candidates() -> Vec<std::path::PathBuf> {
    Vec::new()
}

#[cfg(unix)]
fn path_candidates(paths: impl IntoIterator<Item = &'static str>) -> Vec<std::path::PathBuf> {
    paths.into_iter().map(std::path::PathBuf::from).collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fontconfig_candidates(
    font_names: impl IntoIterator<Item = &'static str>,
) -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    for font_name in font_names {
        let Ok(output) = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", font_name])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let path = std::path::PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if path.as_os_str().is_empty() || candidates.contains(&path) {
            continue;
        }
        candidates.push(path);
    }
    candidates
}

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

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
            setup_system_fonts(ctx);
            setup_ui_style(ctx);
            let app = app::App::new(rt, ctx.clone());
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
