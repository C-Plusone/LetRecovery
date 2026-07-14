//! Small, reusable controls for pages that explicitly opt into the Inno-style theme.

use egui::{Button, Color32, RichText, Stroke, Ui, WidgetText};

use super::inno_theme::{self, Palette, CONTROL_HEIGHT};

pub(crate) fn page_header(ui: &mut Ui, title: impl Into<String>, description: impl Into<String>) {
    let palette = Palette::for_dark_mode(ui.visuals().dark_mode);
    let title = title.into();
    let description = description.into();
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(width, 54.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_width(width);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(14.0).strong().color(palette.text));
                    ui.add_space(1.0);
                    ui.label(
                        RichText::new(description)
                            .size(10.5)
                            .color(palette.secondary_text),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    wizard_icon(ui, 46.0);
                });
            });
        },
    );
    separator(ui);
}

pub(crate) fn dialog_header(ui: &mut Ui, title: impl Into<String>) {
    page_header(ui, title, crate::tr!("配置所选操作并确认后继续。"));
    ui.add_space(8.0);
}

fn wizard_icon(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    let blue = Color32::from_rgb(38, 165, 222);
    let gray = Color32::from_rgb(153, 153, 153);
    let stroke = Stroke::new(1.8, blue);
    let left = rect.left() + size * 0.17;
    let top = rect.top() + size * 0.16;
    let right = rect.right() - size * 0.12;
    let bottom = rect.bottom() - size * 0.12;
    painter.line_segment(
        [
            egui::pos2(left, top),
            egui::pos2(right - size * 0.18, top - size * 0.05),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(left, top),
            egui::pos2(left + size * 0.10, top + size * 0.18),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(left + size * 0.10, top + size * 0.18),
            egui::pos2(right, top + size * 0.11),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(right, top + size * 0.11),
            egui::pos2(right - size * 0.05, bottom),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(right - size * 0.05, bottom),
            egui::pos2(left + size * 0.11, bottom - size * 0.05),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(left + size * 0.11, bottom - size * 0.05),
            egui::pos2(left + size * 0.10, top + size * 0.18),
        ],
        stroke,
    );
    let center = egui::pos2(left + size * 0.15, top + size * 0.52);
    painter.circle_filled(center, size * 0.18, palette_surface(ui));
    painter.circle_stroke(center, size * 0.18, Stroke::new(1.8, gray));
    painter.circle_stroke(center, size * 0.07, Stroke::new(1.5, gray));
}

fn palette_surface(ui: &Ui) -> Color32 {
    Palette::for_dark_mode(ui.visuals().dark_mode).surface
}

pub(crate) fn secondary_button(ui: &mut Ui, text: impl Into<WidgetText>) -> egui::Response {
    ui.add_sized(
        [76.0, CONTROL_HEIGHT],
        Button::new(text).corner_radius(inno_theme::CONTROL_RADIUS),
    )
}

pub(crate) fn primary_button(ui: &mut Ui, text: impl Into<String>) -> egui::Response {
    let dark_mode = ui.visuals().dark_mode;
    let fill = if dark_mode {
        Color32::from_rgb(43, 82, 99)
    } else {
        Color32::from_rgb(0, 95, 184)
    };
    let text_color = Color32::WHITE;
    ui.add_sized(
        [88.0, CONTROL_HEIGHT],
        Button::new(RichText::new(text).color(text_color))
            .fill(fill)
            .stroke(Stroke::new(1.0, Palette::for_dark_mode(dark_mode).focus))
            .corner_radius(inno_theme::CONTROL_RADIUS),
    )
}

pub(crate) fn navigation_item(
    ui: &mut Ui,
    selected: bool,
    text: impl Into<String>,
) -> egui::Response {
    let palette = Palette::for_dark_mode(ui.visuals().dark_mode);
    let text = text.into();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::click());
    let fill = if selected {
        if ui.visuals().dark_mode {
            Color32::from_rgb(56, 56, 56)
        } else {
            Color32::from_rgb(232, 232, 232)
        }
    } else if response.hovered() {
        palette.control_hovered
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect.shrink(1.0), inno_theme::CONTROL_RADIUS, fill);
    if selected {
        let accent = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 2.0, rect.top() + 6.0),
            egui::pos2(rect.left() + 5.0, rect.bottom() - 6.0),
        );
        ui.painter().rect_filled(accent, 2.0, palette.focus);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            inno_theme::CONTROL_RADIUS,
            Stroke::new(1.0, palette.focus),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(12.0),
        palette.text,
    );
    response
}

pub(crate) fn separator(ui: &mut Ui) {
    let palette = Palette::for_dark_mode(ui.visuals().dark_mode);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(
                palette.border.r(),
                palette.border.g(),
                palette.border.b(),
                96,
            ),
        ),
    );
}
