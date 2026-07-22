//! Reversible desktop-wallpaper side effect for the built-in DPRK language easter egg.
//!
//! The image is embedded in the executable. The first activation records the current wallpaper
//! path under LocalAppData before publishing the embedded image. Selecting any other language
//! restores that recorded path. WinPE deliberately does not call this module.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETDESKWALLPAPER,
    SPI_SETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

const WALLPAPER_BYTES: &[u8] = include_bytes!("../../assets/easter_egg/dprk_wallpaper.jpg");
const WALLPAPER_FILE_NAME: &str = "dprk-easter-egg-wallpaper.jpg";
const BACKUP_FILE_NAME: &str = "dprk-easter-egg-wallpaper-backup.json";

#[derive(Debug, Serialize, Deserialize)]
struct WallpaperBackup {
    previous_wallpaper: String,
}

pub fn sync_for_language(language_code: &str) -> Result<()> {
    if crate::utils::i18n::is_dprk_easter_egg_language(language_code) {
        enable()
    } else {
        restore()
    }
}

fn state_directory() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("LetRecovery")
        .join("easter-eggs")
}

fn enable() -> Result<()> {
    let directory = state_directory();
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("创建彩蛋资源目录失败: {}", directory.display()))?;

    let wallpaper_path = directory.join(WALLPAPER_FILE_NAME);
    if std::fs::read(&wallpaper_path).ok().as_deref() != Some(WALLPAPER_BYTES) {
        write_atomic(&wallpaper_path, "dprk-wallpaper", "jpg", WALLPAPER_BYTES)?;
    }

    let backup_path = directory.join(BACKUP_FILE_NAME);
    if backup_path.exists() {
        read_backup(&backup_path)
            .with_context(|| format!("现有壁纸备份无效，拒绝覆盖: {}", backup_path.display()))?;
    } else {
        let backup = WallpaperBackup {
            previous_wallpaper: current_wallpaper().context("读取当前桌面壁纸失败")?,
        };
        let content = serde_json::to_vec_pretty(&backup).context("序列化壁纸备份失败")?;
        write_atomic(&backup_path, "dprk-wallpaper-backup", "json", &content)?;
    }

    set_wallpaper(&wallpaper_path).context("设置朝鲜文彩蛋桌面壁纸失败")
}

fn restore() -> Result<()> {
    let backup_path = state_directory().join(BACKUP_FILE_NAME);
    if !backup_path.exists() {
        return Ok(());
    }

    let backup = read_backup(&backup_path)?;
    set_wallpaper(Path::new(&backup.previous_wallpaper)).context("恢复彩蛋前桌面壁纸失败")?;
    std::fs::remove_file(&backup_path)
        .with_context(|| format!("删除已恢复的壁纸备份失败: {}", backup_path.display()))?;
    Ok(())
}

fn read_backup(path: &Path) -> Result<WallpaperBackup> {
    let content =
        std::fs::read(path).with_context(|| format!("读取壁纸备份失败: {}", path.display()))?;
    serde_json::from_slice(&content)
        .with_context(|| format!("解析壁纸备份失败: {}", path.display()))
}

fn write_atomic(path: &Path, prefix: &str, extension: &str, content: &[u8]) -> Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("目标路径没有父目录: {}", path.display()))?;
    let (temporary, mut file) =
        lr_core::scoped_temp_file::ScopedTempFile::create_writer_in(directory, prefix, extension)
            .with_context(|| format!("创建临时文件失败: {}", directory.display()))?;
    file.write_all(content)
        .with_context(|| format!("写入临时文件失败: {}", temporary.path().display()))?;
    file.flush()
        .with_context(|| format!("刷新临时文件失败: {}", temporary.path().display()))?;
    file.sync_all()
        .with_context(|| format!("同步临时文件失败: {}", temporary.path().display()))?;
    drop(file);
    temporary
        .persist_replace(path)
        .with_context(|| format!("原子发布文件失败: {}", path.display()))
}

fn current_wallpaper() -> Result<String> {
    const BUFFER_LENGTH: usize = 32_768;
    let mut buffer = vec![0_u16; BUFFER_LENGTH];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            BUFFER_LENGTH as u32,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .context("SystemParametersInfoW(SPI_GETDESKWALLPAPER) 失败")?;
    }
    let length = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    Ok(String::from_utf16_lossy(&buffer[..length]))
}

fn set_wallpaper(path: &Path) -> Result<()> {
    let mut wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_mut_ptr().cast::<c_void>()),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
        .context("SystemParametersInfoW(SPI_SETDESKWALLPAPER) 失败")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_wallpaper_is_a_nonempty_jpeg() {
        assert!(WALLPAPER_BYTES.len() > 100_000);
        assert_eq!(&WALLPAPER_BYTES[..2], &[0xff, 0xd8]);
        assert_eq!(&WALLPAPER_BYTES[WALLPAPER_BYTES.len() - 2..], &[0xff, 0xd9]);
    }

    #[test]
    fn normal_languages_do_not_enable_the_easter_egg() {
        assert!(!crate::utils::i18n::is_dprk_easter_egg_language("ko-KR"));
        assert!(crate::utils::i18n::is_dprk_easter_egg_language("KO-kp"));
    }
}
