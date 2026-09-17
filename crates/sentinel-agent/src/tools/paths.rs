//! Path handling for tool inputs: `~` expansion, absolute-only, and lexical normalization that
//! never follows symlinks (trashing a symlink must trash the link, not its target).

use std::path::{Component, Path, PathBuf};

use sentinel_core::{CoreResult, SentinelError};

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Expands `~`, requires an absolute result, and resolves `.` / `..` lexically.
pub fn normalize(raw: &str) -> CoreResult<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SentinelError::invalid("path must not be empty"));
    }
    let expanded = if trimmed == "~" {
        home_dir().ok_or_else(|| SentinelError::invalid("home directory is unknown"))?
    } else if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        home_dir()
            .ok_or_else(|| SentinelError::invalid("home directory is unknown"))?
            .join(rest)
    } else {
        PathBuf::from(trimmed)
    };
    if !expanded.is_absolute() {
        return Err(SentinelError::invalid(format!(
            "path `{trimmed}` must be absolute (or start with ~/)"
        )));
    }
    let mut out = PathBuf::new();
    for component in expanded.components() {
        match component {
            Component::ParentDir => {
                if !out.pop() {
                    return Err(SentinelError::invalid(format!(
                        "path `{trimmed}` escapes the root"
                    )));
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

pub fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `child` is `parent` or lies beneath it (component-wise, so `/a/bc` is not under `/a/b`).
pub fn is_within(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_and_normalizes() {
        let home = home_dir().unwrap();
        assert_eq!(normalize("~").unwrap(), home);
        assert_eq!(
            normalize(" ~/Library/Caches/../Logs ").unwrap(),
            home.join("Library/Logs")
        );
        #[cfg(unix)]
        {
            assert_eq!(normalize("/tmp/./a/b/..").unwrap(), PathBuf::from("/tmp/a"));
            assert!(normalize("/..").is_err());
        }
        assert!(normalize("relative/path").is_err());
        assert!(normalize("").is_err());
    }

    #[test]
    fn containment_is_component_wise() {
        assert!(is_within(Path::new("/a/b/c"), Path::new("/a/b")));
        assert!(is_within(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!is_within(Path::new("/a/bc"), Path::new("/a/b")));
    }
}
