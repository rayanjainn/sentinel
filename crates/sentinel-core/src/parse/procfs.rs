//! Linux `/proc` text formats.

use std::collections::HashMap;

/// `/proc/meminfo` values in bytes (the file reports kB).
pub fn meminfo(text: &str) -> HashMap<String, u64> {
    text.lines()
        .filter_map(|line| {
            let (key, rest) = line.split_once(':')?;
            let mut parts = rest.split_whitespace();
            let value: u64 = parts.next()?.parse().ok()?;
            let bytes = match parts.next() {
                Some("kB") => value.saturating_mul(1024),
                _ => value,
            };
            Some((key.trim().to_owned(), bytes))
        })
        .collect()
}

/// Fields of `/proc/[pid]/stat` Sentinel uses. The command name is parenthesised and may contain
/// spaces or parentheses, so fields are counted from the last `)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PidStat {
    pub state: char,
    pub ppid: u32,
    pub nice: i32,
    pub num_threads: u32,
    pub start_time_ticks: u64,
}

pub fn pid_stat(text: &str) -> Option<PidStat> {
    let close = text.rfind(')')?;
    let fields: Vec<&str> = text[close + 1..].split_whitespace().collect();
    // After the comm field, index 0 is field 3 (state).
    let field = |n: usize| fields.get(n - 3).copied();
    Some(PidStat {
        state: field(3)?.chars().next()?,
        ppid: field(4)?.parse().ok()?,
        nice: field(19)?.parse().ok()?,
        num_threads: field(20)?.parse().ok()?,
        start_time_ticks: field(22)?.parse().ok()?,
    })
}

/// Target of a `/proc/[pid]/fd/N` symlink, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FdTarget {
    Socket(u64),
    Pipe,
    Anon(String),
    Path(String),
}

pub fn fd_target(link: &str) -> FdTarget {
    if let Some(inode) = link
        .strip_prefix("socket:[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|n| n.parse().ok())
    {
        return FdTarget::Socket(inode);
    }
    if link.starts_with("pipe:[") {
        return FdTarget::Pipe;
    }
    if let Some(kind) = link.strip_prefix("anon_inode:") {
        return FdTarget::Anon(kind.to_owned());
    }
    FdTarget::Path(link.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        let text = "MemTotal:       16326412 kB\nMemFree:          812344 kB\nHugePages_Total:       0\nZswap:  1024 kB\n";
        let info = meminfo(text);
        assert_eq!(info["MemTotal"], 16_326_412 * 1024);
        assert_eq!(info["HugePages_Total"], 0);
        assert_eq!(info["Zswap"], 1024 * 1024);
    }

    #[test]
    fn parses_stat_with_awkward_comm() {
        let text = "4242 (tmux: server (1)) S 1 4242 4242 0 -1 4194624 1033 0 0 0 12 5 0 0 20 -5 3 0 987654 12345678 1024 18446744073709551615 1 1 0 0 0 0 0 3 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
        let stat = pid_stat(text).unwrap();
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.ppid, 1);
        assert_eq!(stat.nice, -5);
        assert_eq!(stat.num_threads, 3);
        assert_eq!(stat.start_time_ticks, 987_654);
        assert!(pid_stat("garbage").is_none());
    }

    #[test]
    fn classifies_fd_links() {
        assert_eq!(fd_target("socket:[98765]"), FdTarget::Socket(98765));
        assert_eq!(fd_target("pipe:[12]"), FdTarget::Pipe);
        assert_eq!(
            fd_target("anon_inode:[eventfd]"),
            FdTarget::Anon("[eventfd]".into())
        );
        assert_eq!(
            fd_target("/home/u/a b.txt"),
            FdTarget::Path("/home/u/a b.txt".into())
        );
    }
}
