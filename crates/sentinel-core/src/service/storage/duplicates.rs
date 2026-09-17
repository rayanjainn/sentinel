//! Duplicate detection over a finished scan: group by length, then a head hash, then a full BLAKE3
//! hash, each stage in parallel and cancellable.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;

use super::tree::ScanTree;
use crate::error::{CoreResult, SentinelError};
use crate::model::{DuplicateGroup, DuplicatePhase, NodeId};

const HEAD_BYTES: usize = 16 * 1024;
const BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Default)]
pub struct DuplicateCounters {
    pub candidates: AtomicU64,
    pub hashed: AtomicU64,
    pub bytes_hashed: AtomicU64,
    pub phase: parking_lot::Mutex<Option<DuplicatePhase>>,
}

struct Candidate {
    id: NodeId,
    path: PathBuf,
    len: u64,
}

fn set_phase(counters: &DuplicateCounters, phase: DuplicatePhase) {
    *counters.phase.lock() = Some(phase);
}

fn hash_head(path: &PathBuf, counters: &DuplicateCounters) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; HEAD_BYTES];
    let mut filled = 0;
    while filled < HEAD_BYTES {
        let n = file.read(&mut buffer[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    counters
        .bytes_hashed
        .fetch_add(filled as u64, Ordering::Relaxed);
    Ok(*blake3::hash(&buffer[..filled]).as_bytes())
}

fn hash_full(
    path: &PathBuf,
    cancel: &AtomicBool,
    counters: &DuplicateCounters,
) -> io::Result<Option<[u8; 32]>> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER_BYTES];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        counters.bytes_hashed.fetch_add(n as u64, Ordering::Relaxed);
    }
    Ok(Some(*hasher.finalize().as_bytes()))
}

fn regroup<K: std::hash::Hash + Eq>(items: Vec<(K, Candidate)>) -> Vec<Vec<Candidate>> {
    let mut groups: HashMap<K, Vec<Candidate>> = HashMap::new();
    for (key, candidate) in items {
        groups.entry(key).or_default().push(candidate);
    }
    groups.into_values().filter(|g| g.len() > 1).collect()
}

pub fn find(
    tree: &ScanTree,
    min_size_bytes: u64,
    cancel: &AtomicBool,
    counters: &DuplicateCounters,
) -> CoreResult<Vec<DuplicateGroup>> {
    set_phase(counters, DuplicatePhase::GroupingBySize);
    let mut by_allocated: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for (id, node) in tree.files() {
        if node.size >= min_size_bytes.max(1) {
            by_allocated.entry(node.size).or_default().push(id);
        }
    }
    let ids: Vec<NodeId> = by_allocated
        .into_values()
        .filter(|ids| ids.len() > 1)
        .flatten()
        .collect();
    counters
        .candidates
        .store(ids.len() as u64, Ordering::Relaxed);

    // Re-stat: sizes in the tree are allocations, and files may have changed since the scan.
    let stated: Vec<(u64, Candidate)> = ids
        .into_par_iter()
        .filter_map(|id| {
            let path = tree.path(id);
            let meta = std::fs::symlink_metadata(&path).ok()?;
            (meta.is_file() && meta.len() >= min_size_bytes).then_some((
                meta.len(),
                Candidate {
                    id,
                    path,
                    len: meta.len(),
                },
            ))
        })
        .collect();
    if cancel.load(Ordering::Relaxed) {
        return Err(SentinelError::Cancelled);
    }

    set_phase(counters, DuplicatePhase::HashingHeads);
    let heads: Vec<((u64, [u8; 32]), Candidate)> = regroup(stated)
        .into_par_iter()
        .flatten()
        .filter_map(|candidate| {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let head = hash_head(&candidate.path, counters).ok()?;
            counters.hashed.fetch_add(1, Ordering::Relaxed);
            Some(((candidate.len, head), candidate))
        })
        .collect();
    if cancel.load(Ordering::Relaxed) {
        return Err(SentinelError::Cancelled);
    }

    set_phase(counters, DuplicatePhase::HashingFull);
    let head_groups = regroup(heads);
    counters.hashed.store(0, Ordering::Relaxed);
    let full: Vec<((u64, [u8; 32]), Candidate)> = head_groups
        .into_par_iter()
        .flat_map_iter(|group| {
            let small_enough = group.first().is_some_and(|c| c.len as usize <= HEAD_BYTES);
            group.into_iter().map(move |c| (small_enough, c))
        })
        .filter_map(|(head_is_full, candidate)| {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let hash = if head_is_full {
                hash_head(&candidate.path, counters).ok()?
            } else {
                hash_full(&candidate.path, cancel, counters).ok()??
            };
            counters.hashed.fetch_add(1, Ordering::Relaxed);
            Some(((candidate.len, hash), candidate))
        })
        .collect();
    if cancel.load(Ordering::Relaxed) {
        return Err(SentinelError::Cancelled);
    }

    let mut hashed: HashMap<(u64, [u8; 32]), Vec<Candidate>> = HashMap::new();
    for (key, candidate) in full {
        hashed.entry(key).or_default().push(candidate);
    }
    let mut groups: Vec<DuplicateGroup> = hashed
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|((len, hash), members)| {
            let copies = members.len() as u64;
            let mut files: Vec<_> = members
                .iter()
                .filter_map(|c| tree.file_entry(c.id))
                .collect();
            files.sort_by(|a, b| a.path.cmp(&b.path));
            DuplicateGroup {
                hash: hash.iter().map(|b| format!("{b:02x}")).collect(),
                size_bytes: len,
                reclaimable_bytes: len * (copies - 1),
                files,
            }
        })
        .collect();
    groups.sort_by(|a, b| b.reclaimable_bytes.cmp(&a.reclaimable_bytes));
    Ok(groups)
}
