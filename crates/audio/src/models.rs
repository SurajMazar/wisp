//! Local speech models from `models/manifests` (spec §43, §108, §112): status, verified
//! download with progress and resume, removal. Never downloads anything not in the manifest.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

static STT: &str = include_str!("../../../models/manifests/stt.json");
static TTS: &str = include_str!("../../../models/manifests/tts.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestModel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub preset: Option<String>,
    pub languages: Vec<String>,
    pub license: String,
    pub source: String,
    #[serde(default)]
    pub voices: Vec<String>,
    pub files: Vec<ManifestFile>,
}

#[derive(Deserialize)]
struct Manifest {
    models: Vec<ManifestModel>,
}

pub fn catalog() -> Vec<ManifestModel> {
    [STT, TTS]
        .iter()
        .filter_map(|s| serde_json::from_str::<Manifest>(s).ok())
        .flat_map(|m| m.models)
        .collect()
}

pub fn find(id: &str) -> Option<ManifestModel> {
    catalog().into_iter().find(|m| m.id == id)
}

pub fn model_dir(models_root: &Path, id: &str) -> PathBuf {
    models_root.join(id)
}

/// Installed = every file present with the manifest's exact size (checksums verified at download).
pub fn installed(models_root: &Path, m: &ManifestModel) -> bool {
    m.files.iter().all(|f| {
        std::fs::metadata(model_dir(models_root, &m.id).join(&f.path))
            .map(|md| md.len() == f.size_bytes)
            .unwrap_or(false)
    })
}

pub fn total_size(m: &ManifestModel) -> u64 {
    m.files.iter().map(|f| f.size_bytes).sum()
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("unknown model")]
    Unknown,
    #[error("not enough free disk space")]
    DiskFull,
    #[error("download failed: {0}")]
    Network(String),
    #[error("the downloaded file didn't match its checksum, so it was discarded")]
    Checksum,
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Download (resuming partial files), verify size and SHA-256, then move into place.
pub async fn download(
    models_root: &Path,
    id: &str,
    cancel: CancellationToken,
    mut progress: impl FnMut(u64, u64) + Send,
) -> Result<(), DownloadError> {
    let m = find(id).ok_or(DownloadError::Unknown)?;
    let dir = model_dir(models_root, id);
    tokio::fs::create_dir_all(&dir).await?;
    let total = total_size(&m);
    let client = reqwest::Client::new();
    let mut done_bytes: u64 = 0;
    for f in &m.files {
        if !f.url.starts_with("https://huggingface.co/")
            || f.path.contains("..")
            || f.path.starts_with('/')
        {
            return Err(DownloadError::Unknown);
        }
        let dest = dir.join(&f.path);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if std::fs::metadata(&dest)
            .map(|md| md.len() == f.size_bytes)
            .unwrap_or(false)
        {
            done_bytes += f.size_bytes;
            progress(done_bytes, total);
            continue;
        }
        let partial = dest.with_extension("partial");
        let mut have = tokio::fs::metadata(&partial)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if have > f.size_bytes {
            tokio::fs::remove_file(&partial).await?;
            have = 0;
        }
        let mut req = client.get(&f.url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| DownloadError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(DownloadError::Network(format!("HTTP {}", resp.status())));
        }
        if have > 0 && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            have = 0; // server ignored the range; start over
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(have > 0)
            .write(true)
            .truncate(have == 0)
            .open(&partial)
            .await?;
        let mut stream = resp.bytes_stream();
        let mut got = have;
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
                n = stream.next() => n,
            };
            let Some(chunk) = next else { break };
            let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;
            file.write_all(&chunk).await?;
            got += chunk.len() as u64;
            progress(done_bytes + got, total);
        }
        file.flush().await?;
        drop(file);
        if got != f.size_bytes {
            return Err(DownloadError::Network(format!(
                "incomplete download ({got} of {} bytes)",
                f.size_bytes
            )));
        }
        let p2 = partial.clone();
        let sum = tokio::task::spawn_blocking(move || sha256_file(&p2))
            .await
            .map_err(|e| DownloadError::Network(e.to_string()))??;
        if sum != f.sha256 {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(DownloadError::Checksum);
        }
        tokio::fs::rename(&partial, &dest).await?;
        done_bytes += f.size_bytes;
    }
    Ok(())
}

pub fn remove(models_root: &Path, id: &str) -> std::io::Result<()> {
    if find(id).is_none() {
        return Ok(());
    }
    let dir = model_dir(models_root, id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_parse_with_pinned_huggingface_sources() {
        let c = catalog();
        assert!(
            c.iter()
                .any(|m| m.id == "whisper-base.en" && m.kind == "stt")
        );
        let k = c.iter().find(|m| m.id == "kokoro-82m").expect("kokoro");
        assert!(k.voices.contains(&"af_heart".to_string()));
        for m in &c {
            assert!(m.source.contains('@'), "{} must pin a revision", m.id);
            for f in &m.files {
                assert!(
                    f.url.starts_with("https://huggingface.co/") && f.url.contains("/resolve/")
                );
                assert_eq!(f.sha256.len(), 64);
            }
        }
    }

    #[test]
    fn installed_requires_exact_sizes() {
        let dir = tempfile::tempdir().expect("tmp");
        let m = find("whisper-base.en").expect("model");
        assert!(!installed(dir.path(), &m));
        std::fs::create_dir_all(dir.path().join(&m.id)).expect("mkdir");
        std::fs::write(dir.path().join(&m.id).join(&m.files[0].path), b"short").expect("write");
        assert!(!installed(dir.path(), &m));
    }
}
