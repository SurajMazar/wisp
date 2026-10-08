//! Path validation: canonicalize and confine to allowed roots; sanitize imported names.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PathError {
    #[error("path escapes its allowed root")]
    Escapes,
    #[error("path is not valid: {0}")]
    Invalid(String),
}

/// Join a *relative* user/AI-supplied path onto `root`, rejecting absolute paths, `..`
/// traversal and symlinks that resolve outside `root`. The target need not exist yet;
/// its nearest existing ancestor is canonicalized.
pub fn confine(root: &Path, relative: &str) -> Result<PathBuf, PathError> {
    let rel = Path::new(relative);
    if relative.is_empty() || relative.contains('\0') {
        return Err(PathError::Invalid("empty or NUL".into()));
    }
    for c in rel.components() {
        match c {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(PathError::Escapes),
        }
    }
    let root = root
        .canonicalize()
        .map_err(|e| PathError::Invalid(e.to_string()))?;
    let joined = root.join(rel);

    // Canonicalize the deepest existing ancestor to catch symlink escapes.
    let mut probe = joined.as_path();
    let resolved = loop {
        if let Ok(c) = probe.canonicalize() {
            break c;
        }
        probe = probe.parent().ok_or(PathError::Escapes)?;
    };
    if !resolved.starts_with(&root) {
        return Err(PathError::Escapes);
    }
    Ok(joined)
}

/// Make an imported file name safe to store: strip directories, control characters and
/// leading dots; cap length; never empty.
pub fn sanitize_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    let mut out: String = trimmed.chars().take(200).collect();
    if out.is_empty() {
        out = "file".into();
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn confines_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        assert!(confine(root, "a/b.txt").is_ok());
        assert_eq!(confine(root, "../x"), Err(PathError::Escapes));
        assert_eq!(confine(root, "/etc/passwd"), Err(PathError::Escapes));
        assert_eq!(confine(root, "a/../../x"), Err(PathError::Escapes));
        assert!(confine(root, "").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).expect("symlink");
        assert_eq!(confine(dir.path(), "link/secret"), Err(PathError::Escapes));
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("..hidden"), "hidden");
        assert_eq!(sanitize_file_name("C:\\evil\\a?.txt"), "a.txt");
        assert_eq!(sanitize_file_name("\u{0}\u{7}"), "file");
    }
}
