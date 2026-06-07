use crate::update::format_bytes;
use eframe::egui;

use crate::types::*;

const XBOX_GAME_BAR_PACKAGE: &str = "Microsoft.XboxGamingOverlay";

pub fn info_card<R>(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui) -> R) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(15.0)
                    .strong()
                    .color(egui::Color32::from_gray(230)),
            );
            ui.add_space(6.0);
            content(ui);
        });
}

pub fn info_row(ui: &mut egui::Ui, key: &str, value: &str) {
    let display_value = if value.trim().is_empty() {
        "—"
    } else {
        value
    };
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(140.0, 18.0),
            egui::Label::new(
                egui::RichText::new(key)
                    .size(12.0)
                    .color(egui::Color32::from_gray(150)),
            ),
        );
        ui.label(
            egui::RichText::new(display_value)
                .size(13.0)
                .color(egui::Color32::from_gray(225)),
        );
    });
    ui.add_space(2.0);
}

pub fn setting_row<R>(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    right: impl FnOnce(&mut egui::Ui) -> R,
) {
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    right(ui);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .size(15.0)
                                .strong()
                                .color(egui::Color32::from_gray(230)),
                        );
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new(description)
                                .size(12.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                    });
                });
            });
        });
}

#[derive(PartialEq, Eq)]
pub enum CardAction {
    None,
    Remove,
    Restore,
}

pub fn draw_card(ui: &mut egui::Ui, card: &Card) -> CardAction {
    let mut action = CardAction::None;
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, color, enabled) = match (card.status, card.busy) {
                        (_, true) => ("...", egui::Color32::from_rgb(72, 72, 88), false),
                        (Status::Installed, false) if card.package == XBOX_GAME_BAR_PACKAGE => {
                            ("Отключить", egui::Color32::from_rgb(170, 60, 60), true)
                        }
                        (Status::Installed, false) => {
                            ("Удалить", egui::Color32::from_rgb(170, 60, 60), true)
                        }
                        (Status::NotInstalled, false) if card.package == XBOX_GAME_BAR_PACKAGE => {
                            ("Включить", egui::Color32::from_rgb(56, 130, 90), true)
                        }
                        (Status::NotInstalled, false) => {
                            ("Восстановить", egui::Color32::from_rgb(56, 130, 90), true)
                        }
                        (Status::Unknown, false) => {
                            ("Проверка...", egui::Color32::from_rgb(60, 60, 70), false)
                        }
                    };

                    let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                        .min_size(egui::vec2(120.0, 36.0))
                        .fill(color);
                    if ui.add_enabled(enabled, btn).clicked() {
                        action = match card.status {
                            Status::Installed => CardAction::Remove,
                            Status::NotInstalled => CardAction::Restore,
                            Status::Unknown => CardAction::None,
                        };
                    }

                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.label(
                            egui::RichText::new(&card.title)
                                .size(15.0)
                                .strong()
                                .color(egui::Color32::from_gray(230)),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(&card.description)
                                .size(12.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                        if let Some(log) = &card.log {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(log)
                                    .size(11.0)
                                    .italics()
                                    .color(egui::Color32::from_gray(130)),
                            );
                        }
                    });
                });
            });
        });
    action
}

#[derive(PartialEq, Eq)]
pub enum TelemetryAction {
    None,
    Disable,
    Enable,
}

pub fn draw_telemetry_card(ui: &mut egui::Ui, item: &TelemetryItem) -> TelemetryAction {
    let mut action = TelemetryAction::None;
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, color, enabled) = match (item.status, item.busy) {
                        (_, true) => ("...", egui::Color32::from_rgb(72, 72, 88), false),
                        (TelemetryStatus::Enabled, false) => {
                            ("Отключить", egui::Color32::from_rgb(170, 60, 60), true)
                        }
                        (TelemetryStatus::Disabled, false) => {
                            ("Включить", egui::Color32::from_rgb(56, 130, 90), true)
                        }
                        (TelemetryStatus::Unknown, false) => {
                            ("Проверка...", egui::Color32::from_rgb(60, 60, 70), false)
                        }
                    };

                    let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                        .min_size(egui::vec2(120.0, 36.0))
                        .fill(color);
                    if ui.add_enabled(enabled, btn).clicked() {
                        action = match item.status {
                            TelemetryStatus::Enabled => TelemetryAction::Disable,
                            TelemetryStatus::Disabled => TelemetryAction::Enable,
                            TelemetryStatus::Unknown => TelemetryAction::None,
                        };
                    }

                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let (status_label, status_color) = match item.status {
                            TelemetryStatus::Enabled => {
                                ("Включена", egui::Color32::from_rgb(220, 120, 120))
                            }
                            TelemetryStatus::Disabled => {
                                ("Отключена", egui::Color32::from_rgb(120, 200, 140))
                            }
                            TelemetryStatus::Unknown => {
                                ("Проверяется...", egui::Color32::from_gray(170))
                            }
                        };

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&item.title)
                                    .size(15.0)
                                    .strong()
                                    .color(egui::Color32::from_gray(230)),
                            );
                            ui.label(
                                egui::RichText::new(format!("• {status_label}"))
                                    .size(12.0)
                                    .color(status_color),
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(&item.description)
                                .size(12.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                        if let Some(log) = &item.log {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(log)
                                    .size(11.0)
                                    .italics()
                                    .color(egui::Color32::from_gray(130)),
                            );
                        }
                    });
                });
            });
        });
    action
}

pub fn draw_cleanup_card(ui: &mut egui::Ui, item: &CleanupItem) -> Option<CleanupId> {
    let mut clicked: Option<CleanupId> = None;
    let danger_color = egui::Color32::from_rgb(220, 160, 90);
    let neutral_color = egui::Color32::from_rgb(225, 225, 225);

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(
            1.0,
            if item.danger {
                egui::Color32::from_rgb(110, 80, 50)
            } else {
                egui::Color32::from_rgb(48, 48, 56)
            },
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, color, enabled) = if item.busy {
                        (
                            "...".to_string(),
                            egui::Color32::from_rgb(72, 72, 88),
                            false,
                        )
                    } else {
                        let base = if item.danger {
                            egui::Color32::from_rgb(170, 70, 60)
                        } else {
                            egui::Color32::from_rgb(170, 110, 40)
                        };
                        let nothing_to_clean = matches!(item.size, CleanupSize::Bytes(0));
                        let lbl = if nothing_to_clean {
                            "Пусто".to_string()
                        } else if item.danger {
                            "Выполнить".to_string()
                        } else {
                            "Очистить".to_string()
                        };
                        let col = if nothing_to_clean {
                            egui::Color32::from_rgb(60, 60, 70)
                        } else {
                            base
                        };
                        (lbl, col, !nothing_to_clean)
                    };

                    let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                        .min_size(egui::vec2(120.0, 36.0))
                        .fill(color);
                    if ui.add_enabled(enabled, btn).clicked() {
                        clicked = Some(item.id);
                    }

                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&item.title).size(15.0).strong().color(
                                if item.danger {
                                    danger_color
                                } else {
                                    neutral_color
                                },
                            ));

                            let size_label = match item.size {
                                CleanupSize::Unknown => "• подсчёт...".to_string(),
                                CleanupSize::NotApplicable => "• системная операция".to_string(),
                                CleanupSize::Bytes(n) => {
                                    format!("• {}", format_bytes(n))
                                }
                            };
                            let size_color = match item.size {
                                CleanupSize::Bytes(0) => egui::Color32::from_gray(120),
                                CleanupSize::Bytes(_) => egui::Color32::from_rgb(140, 200, 240),
                                _ => egui::Color32::from_gray(150),
                            };
                            ui.label(egui::RichText::new(size_label).size(12.0).color(size_color));

                            if item.danger {
                                ui.label(
                                    egui::RichText::new("• ⚠ необратимо")
                                        .size(11.0)
                                        .color(danger_color),
                                );
                            }
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(&item.description)
                                .size(12.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                        if let Some(log) = &item.log {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(log)
                                    .size(11.0)
                                    .italics()
                                    .color(egui::Color32::from_gray(130)),
                            );
                        }
                    });
                });
            });
        });
    clicked
}

#[derive(PartialEq, Eq)]
pub enum ServiceAction {
    None,
    Disable,
    Enable,
}

pub fn draw_service_card(ui: &mut egui::Ui, item: &ServiceItem) -> ServiceAction {
    let mut action = ServiceAction::None;
    egui::Frame::default()
        .fill(egui::Color32::from_rgb(34, 34, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 48, 56)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, color, enabled) = match (item.status, item.busy) {
                        (_, true) => ("...", egui::Color32::from_rgb(72, 72, 88), false),
                        (ServiceStatus::Running, false) => {
                            ("Отключить", egui::Color32::from_rgb(170, 60, 60), true)
                        }
                        (ServiceStatus::Stopped, false) => {
                            ("Включить", egui::Color32::from_rgb(56, 130, 90), true)
                        }
                        (ServiceStatus::Disabled, false) => {
                            ("Включить", egui::Color32::from_rgb(56, 130, 90), true)
                        }
                        (ServiceStatus::Unknown, false) => {
                            ("Проверка...", egui::Color32::from_rgb(60, 60, 70), false)
                        }
                    };

                    let btn = egui::Button::new(egui::RichText::new(label).size(13.0))
                        .min_size(egui::vec2(120.0, 36.0))
                        .fill(color);
                    if ui.add_enabled(enabled, btn).clicked() {
                        action = match item.status {
                            ServiceStatus::Running => ServiceAction::Disable,
                            ServiceStatus::Stopped | ServiceStatus::Disabled => {
                                ServiceAction::Enable
                            }
                            ServiceStatus::Unknown => ServiceAction::None,
                        };
                    }

                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let (status_label, status_color) = match item.status {
                            ServiceStatus::Running => {
                                ("Запущена", egui::Color32::from_rgb(120, 200, 140))
                            }
                            ServiceStatus::Stopped => {
                                ("Остановлена", egui::Color32::from_rgb(220, 180, 100))
                            }
                            ServiceStatus::Disabled => {
                                ("Отключена", egui::Color32::from_rgb(140, 200, 240))
                            }
                            ServiceStatus::Unknown => {
                                ("Проверяется...", egui::Color32::from_gray(170))
                            }
                        };

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&item.title)
                                    .size(15.0)
                                    .strong()
                                    .color(egui::Color32::from_gray(230)),
                            );
                            ui.label(
                                egui::RichText::new(format!("• {status_label}"))
                                    .size(12.0)
                                    .color(status_color),
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(&item.description)
                                .size(12.0)
                                .color(egui::Color32::from_gray(170)),
                        );
                        if let Some(log) = &item.log {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(log)
                                    .size(11.0)
                                    .italics()
                                    .color(egui::Color32::from_gray(130)),
                            );
                        }
                    });
                });
            });
        });
    action
}

pub fn nav_button(ui: &mut egui::Ui, item: &NavItem, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let height = 36.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let bg = if selected {
        egui::Color32::from_rgb(56, 90, 170)
    } else if response.hovered() {
        egui::Color32::from_rgb(48, 48, 58)
    } else {
        egui::Color32::from_rgb(36, 36, 42)
    };

    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), bg);

    let text_color = if selected {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_gray(220)
    };

    let pad_left = 12.0;
    let icon_col_w = 24.0;
    let gap = 8.0;

    let icon_size = egui::vec2(22.0, 22.0);
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + pad_left + icon_col_w * 0.5, rect.center().y),
        icon_size,
    );
    if let Some(source) = nav_icon_source(item.icon) {
        egui::Image::new(source)
            .fit_to_exact_size(icon_size)
            .paint_at(ui, icon_rect);
    }

    let label_pos = egui::pos2(rect.left() + pad_left + icon_col_w + gap, rect.center().y);
    ui.painter().text(
        label_pos,
        egui::Align2::LEFT_CENTER,
        item.label,
        egui::FontId::proportional(14.0),
        text_color,
    );

    if item.beta {
        let badge_w = 38.0;
        let badge_h = 18.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(
                rect.right() - badge_w - 8.0,
                rect.center().y - badge_h * 0.5,
            ),
            egui::vec2(badge_w, badge_h),
        );
        ui.painter().rect_filled(
            badge_rect,
            egui::CornerRadius::same(4),
            egui::Color32::from_rgb(170, 110, 40),
        );
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Beta",
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
    }

    response
}

fn nav_icon_source(path: &str) -> Option<egui::ImageSource<'static>> {
    match path {
        "icons/home.png" => Some(egui::include_image!("../icons/home.png")),
        "icons/uwp.png" => Some(egui::include_image!("../icons/uwp.png")),
        "icons/telemetry.png" => Some(egui::include_image!("../icons/telemetry.png")),
        "icons/memory.png" => Some(egui::include_image!("../icons/memory.png")),
        "icons/cleanup.png" => Some(egui::include_image!("../icons/cleanup.png")),
        "icons/services.png" => Some(egui::include_image!("../icons/services.png")),
        "icons/settings.png" => Some(egui::include_image!("../icons/settings.png")),
        "icons/update.png" => Some(egui::include_image!("../icons/update.png")),
        _ => None,
    }
}

pub fn draw_beta_badge(ui: &mut egui::Ui, font_size: f32) {
    let text = "Beta";
    let pad_x = 8.0;
    let pad_y = 3.0;
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(font_size),
        egui::Color32::WHITE,
    );
    let size = galley.size() + egui::vec2(pad_x * 2.0, pad_y * 2.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(4),
        egui::Color32::from_rgb(170, 110, 40),
    );
    ui.painter().galley(
        rect.center() - galley.size() * 0.5,
        galley,
        egui::Color32::WHITE,
    );
}
