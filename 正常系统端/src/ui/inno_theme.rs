//! Visual language inspired by Inno Setup 6.7's Modern Windows 11 wizard.

use egui::{Color32, CornerRadius, FontId, Stroke, Style, TextStyle, Ui, Vec2};

pub(crate) const CONTROL_HEIGHT: f32 = 24.0;
pub(crate) const CONTROL_RADIUS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Palette {
    pub(crate) text: Color32,
    pub(crate) secondary_text: Color32,
    pub(crate) surface: Color32,
    pub(crate) control: Color32,
    pub(crate) control_hovered: Color32,
    pub(crate) control_pressed: Color32,
    pub(crate) border: Color32,
    pub(crate) focus: Color32,
    pub(crate) disabled: Color32,
}

impl Palette {
    pub(crate) fn for_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                text: Color32::from_rgb(243, 243, 243),
                secondary_text: Color32::from_rgb(200, 200, 200),
                surface: Color32::from_rgb(43, 43, 43),
                control: Color32::from_rgb(52, 52, 52),
                control_hovered: Color32::from_rgb(62, 62, 62),
                control_pressed: Color32::from_rgb(47, 47, 47),
                border: Color32::from_rgb(78, 78, 78),
                focus: Color32::from_rgb(96, 205, 255),
                disabled: Color32::from_rgb(126, 126, 126),
            }
        } else {
            Self {
                text: Color32::from_rgb(27, 27, 27),
                secondary_text: Color32::from_rgb(75, 75, 75),
                surface: Color32::from_rgb(250, 250, 250),
                control: Color32::from_rgb(251, 251, 251),
                control_hovered: Color32::from_rgb(246, 246, 246),
                control_pressed: Color32::from_rgb(240, 240, 240),
                border: Color32::from_rgb(141, 141, 141),
                focus: Color32::from_rgb(0, 95, 184),
                disabled: Color32::from_rgb(146, 146, 146),
            }
        }
    }
}

/// Applies the new visual language to one child UI and restores the caller's style on return.
pub(crate) fn scope<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        apply(ui);
        add_contents(ui)
    })
    .inner
}

fn apply(ui: &mut Ui) {
    let dark_mode = ui.visuals().dark_mode;
    apply_to_style(ui.style_mut(), dark_mode);
}

/// Applies the completed desktop migration's compact style to a light or dark base style.
pub(crate) fn apply_to_style(style: &mut Style, dark_mode: bool) {
    let palette = Palette::for_dark_mode(dark_mode);

    style.spacing.item_spacing = Vec2::new(8.0, 5.0);
    style.spacing.button_padding = Vec2::new(10.0, 3.0);
    style.spacing.interact_size.y = CONTROL_HEIGHT;
    style.spacing.combo_width = 180.0;
    style.spacing.indent = 16.0;
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(10.5));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(12.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(12.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.0));

    let radius = CornerRadius::same(CONTROL_RADIUS);
    let visuals = &mut style.visuals;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = if dark_mode {
        Color32::from_rgb(43, 82, 99)
    } else {
        Color32::from_rgb(204, 232, 255)
    };
    visuals.selection.stroke = Stroke::new(1.0, palette.focus);
    visuals.hyperlink_color = palette.focus;
    visuals.faint_bg_color = palette.surface;
    visuals.extreme_bg_color = palette.control;
    visuals.panel_fill = palette.surface;
    visuals.window_fill = palette.surface;
    visuals.window_stroke = Stroke::new(1.0, palette.border);
    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.resize_corner_size = 10.0;
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.bg_fill = palette.control;
    visuals.widgets.inactive.weak_bg_fill = palette.control;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.bg_fill = palette.control_hovered;
    visuals.widgets.hovered.weak_bg_fill = palette.control_hovered;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.focus);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.bg_fill = palette.control_pressed;
    visuals.widgets.active.weak_bg_fill = palette.control_pressed;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.focus);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open = visuals.widgets.hovered;
    visuals.widgets.noninteractive.expansion = 0.0;
    visuals.widgets.inactive.expansion = 0.0;
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.expansion = 0.0;
    visuals.widgets.open.expansion = 0.0;
}

#[cfg(test)]
mod tests {
    use super::Palette;

    #[test]
    fn palettes_preserve_light_and_dark_contrast_roles() {
        let light = Palette::for_dark_mode(false);
        let dark = Palette::for_dark_mode(true);

        assert_ne!(light, dark);
        assert!(light.text.r() < light.surface.r());
        assert!(dark.text.r() > dark.surface.r());
        assert_ne!(light.focus, light.border);
        assert_ne!(dark.focus, dark.border);
    }
}
