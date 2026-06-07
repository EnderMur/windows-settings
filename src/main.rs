#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cleanup;
mod config;
mod icons;
mod logger;
mod memory;
mod powershell;
mod services;
mod system;
mod telemetry;
mod time_win;
mod types;
mod ui;
mod update;
mod uwp;
mod windows_update;

use eframe::egui;
use std::fs;
use std::sync::Arc;

use app::App;
use logger::{LogLevel, Logger};

fn main() -> eframe::Result<()> {
    let logger = Arc::new(Logger::new());
    logger.log(LogLevel::Normal, "=== Application started ===");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 650.0])
            .with_min_inner_size([700.0, 450.0])
            .with_title("Windows Settings"),
        ..Default::default()
    };

    let logger_for_app = logger.clone();
    eframe::run_native(
        "Windows Settings",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(dark_visuals());
            cc.egui_ctx.set_pixels_per_point(1.15);
            let mut app = App::new(logger_for_app);
            app.spawn_initial_status_check(cc.egui_ctx.clone());
            Ok(Box::new(app))
        }),
    )
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    if let Ok(bytes) = fs::read("C:/Windows/Fonts/segoeui.ttf") {
        fonts.font_data.insert(
            "segoe_ui".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.insert(0, "segoe_ui".to_owned());
        }
    }

    if let Ok(bytes) = fs::read("C:/Windows/Fonts/seguisym.ttf") {
        fonts.font_data.insert(
            "segoe_sym".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            prop.push("segoe_sym".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

fn dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(24, 24, 28);
    v.window_fill = egui::Color32::from_rgb(24, 24, 28);
    v.extreme_bg_color = egui::Color32::from_rgb(18, 18, 22);
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(32, 32, 38);
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 48);
    v.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(40, 40, 48);
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(56, 56, 68);
    v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(56, 56, 68);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(72, 72, 88);
    v.widgets.active.weak_bg_fill = egui::Color32::from_rgb(72, 72, 88);
    v.selection.bg_fill = egui::Color32::from_rgb(80, 110, 200);
    v
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
}

pub fn danger_card_frame() -> egui::Frame {
    card_frame().stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(110, 80, 50)))
}

pub fn nav_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(20, 20, 24))
        .inner_margin(egui::Margin::same(12))
}

pub fn central_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(24, 24, 28))
        .inner_margin(egui::Margin::same(16))
}
