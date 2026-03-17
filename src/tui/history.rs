use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap;
use sha2::{Digest, Sha256};

use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct PersistedReachable {
    node_id: u64,
    size: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct PersistedHistory {
    path: String,
    hash: String,
    node_ids: Vec<u64>,
    #[serde(default)]
    reachable: Vec<PersistedReachable>,
}

pub(super) fn history_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("heap-snapshot").join("history.toml"))
}

pub(super) fn file_sha256(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let hash = Sha256::digest(&data);
    Some(format!("{hash:x}"))
}

pub(super) fn load_history(
    snap_path: &Path,
    snap: &HeapSnapshot,
) -> (Vec<NodeOrdinal>, FxHashMap<NodeOrdinal, f64>) {
    let empty = (Vec::new(), FxHashMap::default());
    let hist_path = match history_file_path() {
        Some(p) => p,
        None => return empty,
    };
    let content = match std::fs::read_to_string(&hist_path) {
        Ok(c) => c,
        Err(_) => return empty,
    };
    let persisted: PersistedHistory = match toml::from_str(&content) {
        Ok(p) => p,
        Err(_) => return empty,
    };

    if persisted.path != snap_path.to_string_lossy() {
        return empty;
    }
    if file_sha256(snap_path).as_deref() != Some(&persisted.hash) {
        return empty;
    }

    let history = persisted
        .node_ids
        .iter()
        .filter_map(|&id| snap.node_for_snapshot_object_id(crate::types::NodeId(id)))
        .collect();

    let reachable = persisted
        .reachable
        .iter()
        .filter_map(|r| {
            snap.node_for_snapshot_object_id(crate::types::NodeId(r.node_id))
                .map(|ord| (ord, r.size))
        })
        .collect();

    (history, reachable)
}

pub(super) fn save_history(
    snap_path: &Path,
    history: &[NodeOrdinal],
    reachable_sizes: &FxHashMap<NodeOrdinal, f64>,
    snap: &HeapSnapshot,
) {
    let hist_path = match history_file_path() {
        Some(p) => p,
        None => return,
    };
    let hash = match file_sha256(snap_path) {
        Some(h) => h,
        None => return,
    };
    let node_ids: Vec<u64> = history.iter().map(|&ord| snap.node_id(ord).0).collect();
    let reachable: Vec<PersistedReachable> = reachable_sizes
        .iter()
        .map(|(&ord, &size)| PersistedReachable {
            node_id: snap.node_id(ord).0,
            size,
        })
        .collect();
    let persisted = PersistedHistory {
        path: snap_path.to_string_lossy().into_owned(),
        hash,
        node_ids,
        reachable,
    };
    let content = match toml::to_string(&persisted) {
        Ok(c) => c,
        Err(_) => return,
    };
    if let Some(parent) = hist_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&hist_path, content);
}
