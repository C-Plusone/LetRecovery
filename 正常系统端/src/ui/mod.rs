pub mod about;
pub mod advanced_options;
pub mod download_progress;
pub mod easy_mode;
pub mod embedded_assets;
pub mod hardware_info;
pub mod install_progress;
pub mod online_download;
pub(crate) mod pe_preparation;
pub mod system_backup;
pub mod system_install;
pub mod tools;

// 导出内嵌资源
pub use embedded_assets::{EmbeddedAssets, EmbeddedLogoType};

/// Informational color used for active work such as BitLocker encryption or decryption.
///
/// The light-theme color intentionally meets WCAG AA contrast against white. The
/// previous bright blue/yellow status colors were difficult to read on light
/// backgrounds.
pub(crate) fn activity_text_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(144, 202, 249) // #90CAF9
    } else {
        egui::Color32::from_rgb(21, 101, 192) // #1565C0
    }
}

/// Warning color with sufficient contrast in both built-in themes.
pub(crate) fn warning_text_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(255, 183, 77) // #FFB74D
    } else {
        egui::Color32::from_rgb(154, 91, 0) // #9A5B00
    }
}

#[cfg(test)]
mod color_tests {
    use super::{activity_text_color, warning_text_color};

    #[test]
    fn status_colors_are_theme_specific() {
        assert_eq!(activity_text_color(false).to_array(), [21, 101, 192, 255]);
        assert_eq!(activity_text_color(true).to_array(), [144, 202, 249, 255]);
        assert_eq!(warning_text_color(false).to_array(), [154, 91, 0, 255]);
        assert_eq!(warning_text_color(true).to_array(), [255, 183, 77, 255]);
    }
}
