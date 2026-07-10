use std::path::PathBuf;

use crate::app::{App, Panel, PeDownloadThenAction};
use crate::download::config::OnlinePE;
use crate::tr;
use lr_core::cached_artifact::{
    CachedArtifactError, CachedArtifactPresence, CachedArtifactStatus, CachedArtifactVerification,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PePreparationOutcome {
    Present,
    DownloadScheduled,
    Rejected,
}

impl App {
    /// Inspect a cached PE or schedule a download when no cache entry exists.
    ///
    /// This UI-thread stage validates metadata and path safety only. The final
    /// handoff stage verifies file contents immediately before using the PE;
    /// install and backup do that work off the UI thread.
    pub(crate) fn prepare_pe_for_action(
        &mut self,
        pe: &OnlinePE,
        action: PeDownloadThenAction,
    ) -> PePreparationOutcome {
        match crate::core::pe::PeManager::find_cached_pe(
            &pe.filename,
            pe.sha256.as_deref(),
            pe.md5.as_deref(),
        ) {
            Ok(CachedArtifactPresence::Present {
                path,
                expected_algorithm,
            }) => {
                match expected_algorithm {
                    Some(algorithm) => log::info!(
                        "[PE] Cached PE found; {} verification will run before use: {}",
                        algorithm.name(),
                        path.display()
                    ),
                    None => log::warn!(
                        "[PE] Cached PE has no declared checksum; continuing for legacy compatibility: {}",
                        path.display()
                    ),
                }
                PePreparationOutcome::Present
            }
            Ok(CachedArtifactPresence::Missing) => {
                log::info!(
                    "[PE] PE file is missing; scheduling download: {}",
                    pe.filename
                );
                self.pending_download_url = Some(pe.download_url.clone());
                self.pending_download_filename = Some(pe.filename.clone());
                self.pending_pe_md5 = pe.md5.clone();
                self.pending_pe_sha256 = pe.sha256.clone();
                self.download_save_path = crate::utils::path::get_pe_dir()
                    .to_string_lossy()
                    .into_owned();
                self.pe_download_then_action = Some(action);
                self.current_panel = Panel::DownloadProgress;
                PePreparationOutcome::DownloadScheduled
            }
            Err(error) => {
                log::error!("[PE] Cached PE inspection rejected the operation: {error}");
                let error = describe_cached_pe_error(&error);
                self.show_error(&tr!("缓存的 PE 文件安全校验失败，已停止操作：{}", error));
                PePreparationOutcome::Rejected
            }
        }
    }
}

/// Return a verified PE path, `None` when it is absent, or an error when a
/// present cache entry cannot be trusted.
pub(crate) fn verified_cached_pe_path(
    pe: &OnlinePE,
) -> Result<Option<(PathBuf, CachedArtifactVerification)>, CachedArtifactError> {
    match crate::core::pe::PeManager::check_cached_pe(
        &pe.filename,
        pe.sha256.as_deref(),
        pe.md5.as_deref(),
    ) {
        Ok(CachedArtifactStatus::Missing) => Ok(None),
        Ok(CachedArtifactStatus::Ready { path, verification }) => Ok(Some((path, verification))),
        Err(error) => Err(error),
    }
}

pub(crate) fn require_verified_cached_pe(pe: &OnlinePE) -> Result<PathBuf, String> {
    match verified_cached_pe_path(pe).map_err(|error| describe_cached_pe_error(&error))? {
        Some((path, _)) => Ok(path),
        None => Err(tr!("PE 文件不存在：{}", pe.filename)),
    }
}

fn describe_cached_pe_error(error: &CachedArtifactError) -> String {
    match error {
        CachedArtifactError::InvalidFilename(_) => {
            tr!("PE 配置中的文件名不安全或无效")
        }
        CachedArtifactError::InvalidChecksum(error) => {
            tr!("PE 配置中的 {} 校验值格式无效", error.algorithm.name())
        }
        CachedArtifactError::InspectPath { path, source } => {
            tr!("无法检查缓存的 PE 文件 {}：{}", path.display(), source)
        }
        CachedArtifactError::UnsafeFileType { path } => {
            tr!("缓存路径不是普通文件，已拒绝使用：{}", path.display())
        }
        CachedArtifactError::CalculateHash {
            path,
            algorithm,
            source,
        } => tr!(
            "无法计算缓存 PE 的 {} 校验值（{}）：{}",
            algorithm.name(),
            path.display(),
            source
        ),
        CachedArtifactError::HashMismatch {
            path,
            algorithm,
            expected,
            actual,
        } => tr!(
            "缓存 PE 的 {} 校验不匹配（{}）。预期：{}，实际：{}",
            algorithm.name(),
            path.display(),
            expected,
            actual
        ),
    }
}
