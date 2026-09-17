//! Trash and move actions. Trash is the only way Sentinel removes anything.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{ActionService, Draft, Execution, metric};
use crate::action::{
    Action, ActionPreview, ActionRisk, ItemOutcome, MetricUnit, OutcomeStatus, PreviewTarget,
    Reversibility,
};
use crate::error::{CoreResult, SentinelError};
use crate::model::{Platform, TimestampSecs};
use crate::util::{format_bytes, home_dir, now_secs, path_string, strip_verbatim, tilde_path};

const MAX_PATHS: usize = 10_000;
const LARGE_BATCH_BYTES: u64 = 50 * 1024 * 1024 * 1024;

/// Size facts about a path, from the latest scan or a bounded walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathSize {
    pub bytes: u64,
    pub items: u64,
    pub modified: Option<TimestampSecs>,
    /// False when the walk hit its time budget and `bytes` is a lower bound.
    pub complete: bool,
}

fn trash_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "Recycle Bin",
        _ => "Trash",
    }
}

/// Locations whose removal would break the OS or the user's account.
fn protected(path: &Path, platform: Platform) -> bool {
    if path.parent().is_none() {
        return true;
    }
    if let Some(home) = home_dir()
        && (path == home || home.starts_with(path))
    {
        return true;
    }
    let text = path_string(path);
    match platform {
        Platform::Macos => {
            const EXACT: [&str; 12] = [
                "/Applications",
                "/Library",
                "/System",
                "/Users",
                "/Volumes",
                "/bin",
                "/etc",
                "/opt",
                "/private",
                "/sbin",
                "/usr",
                "/var",
            ];
            const TREES: [&str; 6] = [
                "/System/",
                "/bin/",
                "/sbin/",
                "/private/etc/",
                "/private/var/db/",
                "/Library/Apple/",
            ];
            EXACT.contains(&text.as_str())
                || TREES.iter().any(|t| text.starts_with(t))
                || (text.starts_with("/usr/") && !text.starts_with("/usr/local/"))
        }
        Platform::Linux => {
            const EXACT: [&str; 14] = [
                "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib64", "/opt", "/proc",
                "/root", "/sbin", "/sys", "/usr", "/var",
            ];
            const TREES: [&str; 8] = [
                "/bin/", "/boot/", "/dev/", "/etc/", "/lib/", "/proc/", "/sbin/", "/sys/",
            ];
            EXACT.contains(&text.as_str())
                || TREES.iter().any(|t| text.starts_with(t))
                || (text.starts_with("/usr/") && !text.starts_with("/usr/local/"))
        }
        Platform::Windows => {
            let lower = text.to_ascii_lowercase();
            let rest = lower.get(2..).unwrap_or_default();
            matches!(
                rest,
                "\\" | "\\windows"
                    | "\\users"
                    | "\\program files"
                    | "\\program files (x86)"
                    | "\\programdata"
            ) || rest.starts_with("\\windows\\")
        }
    }
}

/// Absolute, with parent directories resolved but the final component kept as-is (a symlink is
/// acted on itself, never its target).
fn normalize(raw: &str) -> CoreResult<PathBuf> {
    let path = PathBuf::from(raw.trim());
    if !path.is_absolute() {
        return Err(SentinelError::invalid(format!(
            "{raw} is not an absolute path"
        )));
    }
    let name = match path.file_name() {
        Some(name) => name.to_owned(),
        None => return Ok(path),
    };
    let parent = path.parent().unwrap_or(Path::new("/"));
    match std::fs::canonicalize(parent) {
        Ok(parent) => Ok(strip_verbatim(parent).join(name)),
        Err(_) => Ok(path),
    }
}

struct Target {
    path: PathBuf,
    size: Option<PathSize>,
    problem: Option<String>,
}

fn days_ago(modified: Option<TimestampSecs>) -> Option<u64> {
    modified.map(|m| now_secs().saturating_sub(m) / 86_400)
}

fn describe_age(days: u64) -> String {
    match days {
        0 => "last modified today".to_owned(),
        1 => "last modified yesterday".to_owned(),
        d => format!("last modified {d} days ago"),
    }
}

fn collect_targets(
    service: &ActionService,
    paths: &[String],
) -> CoreResult<(Vec<Target>, Vec<String>)> {
    if paths.is_empty() {
        return Err(SentinelError::invalid("no paths were given"));
    }
    if paths.len() > MAX_PATHS {
        return Err(SentinelError::invalid(format!(
            "at most {MAX_PATHS} paths can be handled in one action"
        )));
    }
    let platform = service.config.platform;
    let mut warnings = Vec::new();
    let mut unique: BTreeSet<PathBuf> = BTreeSet::new();
    for raw in paths {
        let path = normalize(raw)?;
        if protected(&path, platform) {
            return Err(SentinelError::invalid(format!(
                "{} is a protected system or account location and cannot be moved",
                path.display()
            )));
        }
        unique.insert(path);
    }
    // Drop paths already covered by a selected parent folder.
    let all: Vec<PathBuf> = unique.iter().cloned().collect();
    let mut nested = 0;
    unique.retain(|path| {
        let covered = all
            .iter()
            .any(|other| other != path && path.starts_with(other));
        nested += usize::from(covered);
        !covered
    });
    if nested > 0 {
        warnings.push(format!(
            "{nested} selected item(s) are inside other selected folders and are included once."
        ));
    }
    let mut targets = Vec::with_capacity(unique.len());
    for path in unique {
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                let size = service.context.path_size(&path);
                targets.push(Target {
                    path,
                    size,
                    problem: None,
                });
            }
            Err(err) => targets.push(Target {
                problem: Some(match err.kind() {
                    std::io::ErrorKind::NotFound => {
                        "No longer exists; it will be skipped".to_owned()
                    }
                    _ => format!("Cannot be read ({err}); it will be skipped"),
                }),
                path,
                size: None,
            }),
        }
    }
    if targets.iter().all(|t| t.problem.is_some()) {
        return Err(SentinelError::PathNotFound {
            path: path_string(&targets[0].path),
        });
    }
    Ok((targets, warnings))
}

fn preview_targets(targets: &[Target]) -> Vec<PreviewTarget> {
    targets
        .iter()
        .map(|t| PreviewTarget {
            label: tilde_path(&t.path),
            detail: t.size.and_then(|s| {
                let mut parts = Vec::new();
                if s.items > 1 {
                    parts.push(format!("{} items", s.items));
                }
                if let Some(days) = days_ago(s.modified) {
                    parts.push(describe_age(days));
                }
                (!parts.is_empty()).then(|| parts.join(", "))
            }),
            size_bytes: t.size.map(|s| s.bytes),
            problem: t.problem.clone(),
            safety_note: None,
        })
        .collect()
}

fn totals(targets: &[Target]) -> (u64, u64, Option<TimestampSecs>, bool) {
    targets.iter().filter(|t| t.problem.is_none()).fold(
        (0u64, 0u64, None, true),
        |(bytes, items, newest, complete), t| match t.size {
            Some(s) => (
                bytes + s.bytes,
                items + s.items.max(1),
                crate::service::storage::tree::newer(newest, s.modified),
                complete && s.complete,
            ),
            None => (bytes, items + 1, newest, false),
        },
    )
}

fn size_phrase(count: usize, bytes: u64, single: Option<&Path>) -> String {
    match single {
        Some(path) if count == 1 => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path_string(path));
            format!("{name} ({})", format_bytes(bytes))
        }
        _ => format!("{count} items ({})", format_bytes(bytes)),
    }
}

fn common_warnings(targets: &[Target], warnings: &mut Vec<String>) -> bool {
    let mut risky = false;
    for target in targets.iter().filter(|t| t.problem.is_none()) {
        let name = target.path.to_string_lossy();
        if name.ends_with(".app") {
            warnings.push(format!(
                "{} is an application bundle.",
                tilde_path(&target.path)
            ));
            risky = true;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // SAFETY: geteuid has no preconditions.
            let euid = unsafe { libc::geteuid() };
            if let Ok(meta) = std::fs::symlink_metadata(&target.path)
                && meta.uid() != euid
                && euid != 0
            {
                warnings.push(format!(
                    "{} is owned by another user; the system may refuse to move it.",
                    tilde_path(&target.path)
                ));
            }
        }
        if target.size.is_some_and(|s| !s.complete) {
            warnings.push(format!(
                "{} is still being measured; its size shown is a lower bound.",
                tilde_path(&target.path)
            ));
        }
    }
    risky
}

pub(super) fn preview_trash(service: &ActionService, paths: Vec<String>) -> CoreResult<Draft> {
    let (targets, mut warnings) = collect_targets(service, &paths)?;
    let platform = service.config.platform;
    let trash = trash_name(platform);
    let valid: Vec<&Target> = targets.iter().filter(|t| t.problem.is_none()).collect();
    let (bytes, items, newest, _) = totals(&targets);
    let risky = common_warnings(&targets, &mut warnings);
    let first = valid.first().map(|t| t.path.as_path());
    let volume = first.and_then(|p| service.context.volume_label(p));
    let what = size_phrase(valid.len(), bytes, first);
    let mut impact = vec![
        metric("sizeBytes", "Size", bytes as f64, MetricUnit::Bytes),
        metric("itemCount", "Items", items as f64, MetricUnit::Count),
    ];
    if let Some(days) = days_ago(newest) {
        impact.push(metric(
            "daysSinceModified",
            "Days since last modified",
            days as f64,
            MetricUnit::Count,
        ));
    }
    Ok(Draft {
        action: Action::TrashPaths {
            paths: valid.iter().map(|t| path_string(&t.path)).collect(),
        },
        title: format!("Move {what} to {trash}"),
        description: format!(
            "Moves {what} to the {trash}{}. Nothing is deleted: items can be put back from the {trash} \
             until it is emptied, and the space is only released then.",
            volume.map(|v| format!(" on {v}")).unwrap_or_default()
        ),
        targets: preview_targets(&targets),
        impact,
        estimated_bytes_freed: Some(bytes),
        risk: if risky || bytes >= LARGE_BATCH_BYTES || items > 1000 {
            ActionRisk::High
        } else {
            ActionRisk::Moderate
        },
        reversibility: Reversibility::Recoverable {
            how: format!("Put the items back from the {trash}"),
        },
        warnings,
        requires_elevation: false,
    })
}

pub(super) fn preview_move(
    service: &ActionService,
    paths: Vec<String>,
    destination: String,
) -> CoreResult<Draft> {
    let destination = normalize(&destination)?;
    let meta = std::fs::metadata(&destination)
        .map_err(|err| SentinelError::io(&err, Some(&destination)))?;
    if !meta.is_dir() {
        return Err(SentinelError::invalid(format!(
            "{} is not a folder",
            destination.display()
        )));
    }
    let destination = std::fs::canonicalize(&destination)
        .map(strip_verbatim)
        .map_err(|err| SentinelError::io(&err, Some(&destination)))?;
    let (mut targets, mut warnings) = collect_targets(service, &paths)?;
    for target in targets.iter_mut().filter(|t| t.problem.is_none()) {
        if destination.starts_with(&target.path) {
            return Err(SentinelError::invalid(format!(
                "{} cannot be moved into itself",
                target.path.display()
            )));
        }
        if target.path.parent() == Some(destination.as_path()) {
            target.problem =
                Some("Already in the destination folder; it will be skipped".to_owned());
            continue;
        }
        if let Some(name) = target.path.file_name()
            && std::fs::symlink_metadata(destination.join(name)).is_ok()
        {
            target.problem = Some(
                "An item with the same name exists in the destination; it will be skipped"
                    .to_owned(),
            );
        }
    }
    if targets.iter().all(|t| t.problem.is_some()) {
        return Err(SentinelError::invalid(
            "none of the selected items can be moved to that folder",
        ));
    }
    let valid: Vec<&Target> = targets.iter().filter(|t| t.problem.is_none()).collect();
    let (bytes, items, _, _) = totals(&targets);
    let risky = common_warnings(&targets, &mut warnings);
    let first = valid.first().map(|t| t.path.as_path());
    let what = size_phrase(valid.len(), bytes, first);
    let cross_volume = valid
        .iter()
        .any(|t| service.context.same_volume(&t.path, &destination) == Some(false));
    let mut high = risky;
    if cross_volume {
        let trash = trash_name(service.config.platform);
        warnings.push(format!(
            "The destination is on another volume: items are copied, then the originals are moved to the {trash}."
        ));
        if let Ok((_, available)) = service.context.volume_usage(&destination)
            && available < bytes
        {
            warnings.push(format!(
                "The destination has only {} free; the move will stop when it fills up.",
                format_bytes(available)
            ));
            high = true;
        }
    }
    let dest_label = tilde_path(&destination);
    Ok(Draft {
        action: Action::MovePaths {
            paths: valid.iter().map(|t| path_string(&t.path)).collect(),
            destination_dir: path_string(&destination),
        },
        title: format!("Move {what} to {dest_label}"),
        description: if cross_volume {
            format!(
                "Copies {what} into {dest_label}, verifies each copy, then moves the originals to the {}.",
                trash_name(service.config.platform)
            )
        } else {
            format!("Moves {what} into {dest_label} on the same volume; no data is copied.")
        },
        targets: preview_targets(&targets),
        impact: vec![
            metric("sizeBytes", "Size", bytes as f64, MetricUnit::Bytes),
            metric("itemCount", "Items", items as f64, MetricUnit::Count),
        ],
        estimated_bytes_freed: cross_volume.then_some(bytes),
        risk: if high {
            ActionRisk::High
        } else {
            ActionRisk::Moderate
        },
        reversibility: Reversibility::Undoable {
            how: format!("Move the items back out of {dest_label}"),
        },
        warnings,
        requires_elevation: false,
    })
}

fn usage_metrics(service: &ActionService, path: &Path, prefix: &str, label: &str) -> Vec<Metric> {
    match service.context.volume_usage(path) {
        Ok((total, available)) if total > 0 => {
            let used = total.saturating_sub(available) as f64 / total as f64 * 100.0;
            vec![
                metric(
                    &format!("{prefix}UsedPercent"),
                    &format!("{label} used"),
                    (used * 10.0).round() / 10.0,
                    MetricUnit::Percent,
                ),
                metric(
                    &format!("{prefix}FreeBytes"),
                    &format!("{label} free"),
                    available as f64,
                    MetricUnit::Bytes,
                ),
            ]
        }
        _ => Vec::new(),
    }
}

use crate::action::Metric;

fn status_of(items: &[ItemOutcome]) -> OutcomeStatus {
    let ok = items.iter().filter(|i| i.success).count();
    if ok == items.len() {
        OutcomeStatus::Succeeded
    } else if ok == 0 {
        OutcomeStatus::Failed
    } else {
        OutcomeStatus::PartiallySucceeded
    }
}

fn target_sizes(preview: &ActionPreview) -> std::collections::HashMap<String, u64> {
    preview
        .targets
        .iter()
        .filter_map(|t| t.size_bytes.map(|s| (t.label.clone(), s)))
        .collect()
}

fn used_percent(metrics: &[Metric]) -> Option<f64> {
    metrics
        .iter()
        .find(|m| m.key.ends_with("UsedPercent"))
        .map(|m| m.value)
}

fn free_bytes(metrics: &[Metric]) -> Option<f64> {
    metrics
        .iter()
        .find(|m| m.key.ends_with("FreeBytes"))
        .map(|m| m.value)
}

pub(super) fn execute_trash(
    service: &ActionService,
    preview: &ActionPreview,
    paths: &[String],
) -> Execution {
    let platform = service.config.platform;
    let trash = trash_name(platform);
    let Some(first) = paths.first().map(PathBuf::from) else {
        return Execution::failed(
            preview.title.clone(),
            SentinelError::invalid("no paths"),
            Vec::new(),
        );
    };
    let volume = service.context.volume_label(&first);
    let volume_path = first.parent().unwrap_or(&first).to_path_buf();
    let before = usage_metrics(
        service,
        &volume_path,
        "volume",
        volume.as_deref().unwrap_or("Volume"),
    );
    let sizes = target_sizes(preview);
    let fops = match service.context.file_ops() {
        Ok(fops) => fops,
        Err(err) => return Execution::failed(preview.title.clone(), err, before),
    };
    let mut items = Vec::with_capacity(paths.len());
    let mut moved = Vec::new();
    let mut moved_bytes = 0u64;
    for raw in paths {
        let path = PathBuf::from(raw);
        let label = tilde_path(&path);
        match fops.trash(&path) {
            Ok(()) => {
                moved_bytes += sizes.get(&label).copied().unwrap_or(0);
                moved.push(raw.clone());
                items.push(ItemOutcome {
                    label,
                    path: Some(raw.clone()),
                    success: true,
                    error: None,
                });
            }
            Err(err) => items.push(ItemOutcome {
                label,
                path: Some(raw.clone()),
                success: false,
                error: Some(err.into()),
            }),
        }
    }
    let after = usage_metrics(
        service,
        &volume_path,
        "volume",
        volume.as_deref().unwrap_or("Volume"),
    );
    let status = status_of(&items);
    let failed = items.iter().filter(|i| !i.success).count();
    let freed = match (free_bytes(&before), free_bytes(&after)) {
        (Some(b), Some(a)) if a > b => (a - b) as u64,
        _ => 0,
    };
    let mut summary = if moved.is_empty() {
        format!("Nothing was moved to the {trash}.")
    } else {
        format!(
            "Moved {} ({}) to the {trash}.",
            if moved.len() == 1 {
                "1 item".to_owned()
            } else {
                format!("{} items", moved.len())
            },
            format_bytes(moved_bytes)
        )
    };
    if let (Some(name), Some(used)) = (volume.as_deref(), used_percent(&after)) {
        if moved_bytes > 0 && freed >= moved_bytes / 10 * 9 {
            summary.push_str(&format!(
                " Freed {}; {name} is now {used:.0}% used.",
                format_bytes(freed)
            ));
        } else if !moved.is_empty() {
            summary.push_str(&format!(
                " {name} is {used:.0}% used; the space is released when the {trash} is emptied."
            ));
        }
    }
    if failed > 0 {
        summary.push_str(&format!(" {failed} item(s) could not be moved."));
    }
    Execution {
        status,
        items,
        before,
        after,
        summary,
        bytes_freed: (moved_bytes > 0).then_some(moved_bytes),
        affected_paths: moved,
    }
}

pub(super) fn execute_move(
    service: &ActionService,
    preview: &ActionPreview,
    paths: &[String],
    destination: &str,
) -> Execution {
    let destination = PathBuf::from(destination);
    let Some(first) = paths.first().map(PathBuf::from) else {
        return Execution::failed(
            preview.title.clone(),
            SentinelError::invalid("no paths"),
            Vec::new(),
        );
    };
    let source_dir = first.parent().unwrap_or(&first).to_path_buf();
    let snapshot = |service: &ActionService| {
        let mut metrics = usage_metrics(service, &source_dir, "source", "Source volume");
        metrics.extend(usage_metrics(
            service,
            &destination,
            "destination",
            "Destination volume",
        ));
        metrics
    };
    let before = snapshot(service);
    let sizes = target_sizes(preview);
    let fops = match service.context.file_ops() {
        Ok(fops) => fops,
        Err(err) => return Execution::failed(preview.title.clone(), err, before),
    };
    let mut items = Vec::with_capacity(paths.len());
    let mut moved = Vec::new();
    let mut moved_bytes = 0u64;
    for raw in paths {
        let path = PathBuf::from(raw);
        let label = tilde_path(&path);
        match fops.move_into(&path, &destination) {
            Ok(_) => {
                moved_bytes += sizes.get(&label).copied().unwrap_or(0);
                moved.push(raw.clone());
                items.push(ItemOutcome {
                    label,
                    path: Some(raw.clone()),
                    success: true,
                    error: None,
                });
            }
            Err(err) => items.push(ItemOutcome {
                label,
                path: Some(raw.clone()),
                success: false,
                error: Some(err.into()),
            }),
        }
    }
    let after = snapshot(service);
    let failed = items.iter().filter(|i| !i.success).count();
    let mut summary = format!(
        "Moved {} ({}) to {}.",
        if moved.len() == 1 {
            "1 item".to_owned()
        } else {
            format!("{} items", moved.len())
        },
        format_bytes(moved_bytes),
        tilde_path(&destination)
    );
    if failed > 0 {
        summary.push_str(&format!(" {failed} item(s) could not be moved."));
    }
    Execution {
        status: status_of(&items),
        items,
        before,
        after,
        summary,
        bytes_freed: None,
        affected_paths: moved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_system_and_account_roots() {
        assert!(protected(Path::new("/"), Platform::Macos));
        assert!(protected(Path::new("/System/Library"), Platform::Macos));
        assert!(protected(Path::new("/usr/bin"), Platform::Macos));
        assert!(!protected(
            Path::new("/usr/local/lib/node_modules"),
            Platform::Macos
        ));
        assert!(protected(Path::new("/etc/hosts"), Platform::Linux));
        assert!(protected(
            Path::new(r"C:\Windows\System32"),
            Platform::Windows
        ));
        assert!(protected(Path::new(r"C:\Program Files"), Platform::Windows));
        assert!(!protected(
            Path::new(r"C:\Users\me\Downloads\x.zip"),
            Platform::Windows
        ));
        if let Some(home) = home_dir() {
            assert!(protected(&home, Platform::Linux));
            assert!(!protected(&home.join("Downloads/old.dmg"), Platform::Linux));
        }
    }

    #[test]
    fn ages_are_described() {
        assert_eq!(describe_age(0), "last modified today");
        assert_eq!(describe_age(47), "last modified 47 days ago");
    }
}
