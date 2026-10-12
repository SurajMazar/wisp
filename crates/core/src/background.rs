//! Background tools the engine runs itself: files in the user's home and reading web pages.
//! (Reminders, Calendar, Notes, Music and the Trash go through the host's `host.apps.perform`.)
//!
//! Paths are `~/…`; they're resolved, symlinks followed, and must stay inside the home folder,
//! outside `~/Library` and hidden folders. Nothing is ever overwritten.

use std::path::{Path, PathBuf};

use wisp_ipc::{
    IpcError,
    types::{
        settings::PrivacyMode,
        tools::{MoveArgs, PathArgs, ToolExecution, UrlArgs, Verification, VerificationStatus},
    },
};

use crate::Core;

const MAX_LIST: usize = 200;
const MAX_PAGE_BYTES: usize = 2_000_000;
const MAX_PAGE_TEXT: usize = 6_000;

fn home() -> Result<PathBuf, IpcError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| IpcError::internal("no home folder"))
}

fn ok(detail: impl Into<String>, output: Option<String>) -> ToolExecution {
    ToolExecution {
        output,
        verification: Verification {
            status: VerificationStatus::Verified,
            detail: detail.into(),
        },
    }
}

/// `~/x` → an absolute path that really is inside the home folder (after following symlinks).
fn resolve(p: &str, must_exist: bool) -> Result<PathBuf, IpcError> {
    wisp_computer::home_path(p).map_err(|e| IpcError::invalid_params(e.to_string()))?;
    let home = home()?
        .canonicalize()
        .map_err(|e| IpcError::internal(e.to_string()))?;
    let rel = p.trim_start_matches('~').trim_start_matches('/');
    let full = home.join(rel);
    let real = if must_exist {
        full.canonicalize()
            .map_err(|_| IpcError::invalid_params(format!("{p} doesn't exist")))?
    } else {
        // The nearest existing ancestor decides where it really is.
        let mut existing = full.as_path();
        while !existing.exists() {
            existing = existing.parent().unwrap_or(&home);
        }
        let base = existing
            .canonicalize()
            .map_err(|e| IpcError::internal(e.to_string()))?;
        base.join(full.strip_prefix(existing).unwrap_or(Path::new("")))
    };
    let library = home.join("Library");
    if !real.starts_with(&home) || real.starts_with(&library) {
        return Err(IpcError::invalid_params(format!(
            "{p} leads outside your home folder"
        )));
    }
    Ok(real)
}

fn shown(p: &Path) -> String {
    match home()
        .ok()
        .and_then(|h| p.strip_prefix(h).ok().map(Path::to_path_buf))
    {
        Some(rel) => format!("~/{}", rel.display()),
        None => p.display().to_string(),
    }
}

impl Core {
    pub(crate) async fn list_files(&self, a: &PathArgs) -> Result<ToolExecution, IpcError> {
        let dir = resolve(&a.path, true)?;
        let path = a.path.clone();
        tokio::task::spawn_blocking(move || {
            let mut entries: Vec<(bool, String, u64)> = std::fs::read_dir(&dir)
                .map_err(|e| IpcError::invalid_params(format!("can't read {path}: {e}")))?
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') {
                        return None;
                    }
                    let md = e.metadata().ok()?;
                    Some((md.is_dir(), name, md.len()))
                })
                .collect();
            entries.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
            });
            let total = entries.len();
            let mut out: Vec<String> = entries
                .iter()
                .take(MAX_LIST)
                .map(|(dir, name, size)| {
                    if *dir {
                        format!("{name}/")
                    } else {
                        format!("{name} ({} KB)", size.div_ceil(1024))
                    }
                })
                .collect();
            if total > MAX_LIST {
                out.push(format!("… and {} more", total - MAX_LIST));
            }
            Ok(ok(
                format!("{total} items in {path}"),
                Some(if out.is_empty() {
                    "(empty)".into()
                } else {
                    out.join("\n")
                }),
            ))
        })
        .await
        .map_err(|e| IpcError::internal(e.to_string()))?
    }

    pub(crate) async fn move_file(&self, a: &MoveArgs) -> Result<ToolExecution, IpcError> {
        let from = resolve(&a.from, true)?;
        let to_dir = resolve(&a.to, false)?;
        if to_dir.starts_with(&from) {
            return Err(IpcError::invalid_params("can't move a folder into itself"));
        }
        let name = from
            .file_name()
            .ok_or_else(|| IpcError::invalid_params("nothing to move"))?
            .to_owned();
        let target = to_dir.join(&name);
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&to_dir)
                .map_err(|e| IpcError::invalid_params(format!("can't create the folder: {e}")))?;
            if target.exists() {
                return Err(IpcError::invalid_params(format!(
                    "{} already exists, so nothing was moved",
                    shown(&target)
                )));
            }
            std::fs::rename(&from, &target)
                .map_err(|e| IpcError::invalid_params(format!("couldn't move it: {e}")))?;
            Ok(ok(format!("Moved to {}", shown(&target)), None))
        })
        .await
        .map_err(|e| IpcError::internal(e.to_string()))?
    }

    pub(crate) async fn fetch_page(&self, a: &UrlArgs) -> Result<ToolExecution, IpcError> {
        let settings = self.blocking(|db| db.load_settings()).await?;
        if settings.privacy.mode == PrivacyMode::LocalOnly {
            return Err(IpcError::new(
                wisp_ipc::ErrorCode::PermissionDenied,
                "Reading web pages is off in Local Only mode (nothing leaves this Mac).",
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| IpcError::internal(e.to_string()))?;
        let resp = client
            .get(&a.url)
            .header("User-Agent", "Wisp (local assistant)")
            .send()
            .await
            .map_err(|e| {
                IpcError::new(
                    wisp_ipc::ErrorCode::Unavailable,
                    format!("couldn't load the page: {e}"),
                )
            })?;
        if !resp.status().is_success() {
            return Err(IpcError::new(
                wisp_ipc::ErrorCode::Unavailable,
                format!("the page answered {}", resp.status()),
            ));
        }
        let kind = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if !(kind.contains("html") || kind.starts_with("text/") || kind.is_empty()) {
            return Err(IpcError::invalid_params(format!(
                "that's not a web page ({kind})"
            )));
        }
        let mut body = Vec::new();
        let mut stream = resp.bytes_stream();
        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| IpcError::internal(e.to_string()))?;
            body.extend_from_slice(&chunk);
            if body.len() > MAX_PAGE_BYTES {
                break;
            }
        }
        let raw = String::from_utf8_lossy(&body);
        let text = if kind.contains("html") || raw.trim_start().starts_with('<') {
            wisp_files::html_to_text(&raw)
        } else {
            raw.into_owned()
        };
        let n = text.chars().count();
        let excerpt: String = text.chars().take(MAX_PAGE_TEXT).collect();
        // What the page says is data from the web, not instructions.
        Ok(ok(
            format!("Read {n} characters"),
            Some(format!("\"\"\"\n{excerpt}\n\"\"\"")),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_in_home_and_out_of_library() {
        let home = home().expect("home").canonicalize().expect("canon");
        assert_eq!(
            resolve("~/Desktop", false).expect("ok"),
            home.join("Desktop")
        );
        for bad in ["/etc", "~/../x", "~/Library/Keychains", "~/.ssh", "Desktop"] {
            assert!(resolve(bad, false).is_err(), "{bad}");
        }
    }

    #[test]
    fn symlinks_out_of_home_are_refused() {
        let dir = tempfile::Builder::new()
            .prefix("wisp-test-")
            .tempdir_in(home().expect("home"))
            .expect("tmp in home");
        let link = dir.path().join("escape");
        std::os::unix::fs::symlink("/etc", &link).expect("symlink");
        let rel = format!(
            "~/{}",
            link.strip_prefix(home().expect("home"))
                .expect("rel")
                .display()
        );
        assert!(
            resolve(&rel, true).is_err(),
            "a link to /etc is outside home"
        );
    }
}
