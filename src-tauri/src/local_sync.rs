// Bidirectional sync — merges conversations between local agents and a shared folder.
//
// Sync algorithm:
// 1. Read all local conversations (from each agent adapter)
// 2. Read all remote conversations (from the sync folder)
// 3. For each conversation ID:
//    - Only local → upload to sync folder
//    - Only remote → download to local agent storage
//    - Both exist → compare updated_at, keep the newer one
// 4. Write manifest with sync metadata

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Encode a conversation ID into a safe filename.
/// Windows forbids `:` (and several other chars) in filenames.
/// We encode them as HTML entities: `:` → `&#x3a;` etc.
pub fn id_to_filename(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        match ch {
            ':' => out.push_str("&#x3a;"),
            '<' => out.push_str("&#x3c;"),
            '>' => out.push_str("&#x3e;"),
            '"' => out.push_str("&#x22;"),
            '|' => out.push_str("&#x7c;"),
            '?' => out.push_str("&#x3f;"),
            '*' => out.push_str("&#x2a;"),
            '/' => out.push_str("&#x2f;"),
            '\\' => out.push_str("&#x5c;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Decode a safe filename back to the original conversation ID.
fn filename_to_id(name: &str) -> String {
    name.replace("&#x3a;", ":")
        .replace("&#x3c;", "<")
        .replace("&#x3e;", ">")
        .replace("&#x22;", "\"")
        .replace("&#x7c;", "|")
        .replace("&#x3f;", "?")
        .replace("&#x2a;", "*")
        .replace("&#x2f;", "/")
        .replace("&#x5c;", "\\")
}

/// A single conversation file for sync transfer.
#[derive(Debug, Clone)]
pub struct SyncItem {
    pub agent: String,
    pub id: String,
    pub updated_at: String,
    pub body: Vec<u8>,
}

/// Result of a bidirectional sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub uploaded: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub conflicts_resolved: usize,
    pub folder_path: String,
}

/// Status of the sync folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub available: bool,
    pub folder_path: String,
    pub remote_conversation_count: usize,
    pub last_sync_info: Option<String>,
}

/// Check if a sync folder is configured and accessible.
pub fn check_sync_status(folder: &Path) -> SyncStatus {
    if !folder.exists() {
        return SyncStatus {
            available: false,
            folder_path: folder.to_string_lossy().to_string(),
            remote_conversation_count: 0,
            last_sync_info: None,
        };
    }

    let conversations_dir = folder.join("conversations");
    let count = count_remote_conversations(&conversations_dir);
    let last_sync = read_manifest_sync_time(folder);

    SyncStatus {
        available: true,
        folder_path: folder.to_string_lossy().to_string(),
        remote_conversation_count: count,
        last_sync_info: last_sync,
    }
}

fn count_remote_conversations(dir: &Path) -> usize {
    if !dir.exists() {
        return 0;
    }
    let mut count = 0;
    for agent in AGENT_LIST {
        let agent_dir = dir.join(agent);
        if agent_dir.exists() {
            if let Ok(entries) = fs::read_dir(&agent_dir) {
                count += entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "json")
                            .unwrap_or(false)
                    })
                    .count();
            }
        }
    }
    count
}

fn read_manifest_sync_time(folder: &Path) -> Option<String> {
    let manifest_path = folder.join("manifest.json");
    if !manifest_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&content).ok()?;
    manifest
        .get("last_synced_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Read all remote conversations from the sync folder.
/// Returns a map of (agent, id) → (updated_at, body).
pub fn read_remote_conversations(folder: &Path) -> HashMap<(String, String), (String, Vec<u8>)> {
    let mut remote = HashMap::new();
    let conversations_dir = folder.join("conversations");
    if !conversations_dir.exists() {
        return remote;
    }

    for agent in AGENT_LIST {
        let agent_dir = conversations_dir.join(agent);
        if !agent_dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&agent_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map(|e| e != "json").unwrap_or(true) {
                    continue;
                }
                let file_name = path
                    .file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if file_name.is_empty() {
                    continue;
                }
                // Decode HTML entities back to original ID
                let id = filename_to_id(&file_name);

                match fs::read(&path) {
                    Ok(body) => {
                        // Extract updated_at from the JSON
                        let updated_at = extract_updated_at(&body);
                        remote.insert((agent.to_string(), id), (updated_at, body));
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to read remote {}: {e}", path.display());
                    }
                }
            }
        }
    }

    remote
}

/// Extract updated_at from a conversation JSON blob.
fn extract_updated_at(body: &[u8]) -> String {
    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
        val.get("updated_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    }
}

/// Parse an ISO 8601 timestamp string into epoch seconds for comparison.
fn parse_timestamp(ts: &str) -> i64 {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        dt.timestamp()
    } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.fZ") {
        dt.and_utc().timestamp()
    } else {
        0
    }
}

const AGENT_LIST: &[&str] = &["claude", "codex", "gemini", "opencode", "zcode", "hermes"];

/// Perform bidirectional sync between local agents and a sync folder.
///
/// - `local_items`: conversations collected from local agent adapters
/// - `folder`: the user-chosen sync folder path
///
/// Returns the sync result with counts of uploaded/downloaded/skipped items.
pub fn bidirectional_sync(local_items: &[SyncItem], folder: &Path) -> Result<SyncResult> {
    // Ensure folder structure exists
    let conversations_dir = folder.join("conversations");
    fs::create_dir_all(&conversations_dir)?;
    for agent in AGENT_LIST {
        fs::create_dir_all(conversations_dir.join(agent))?;
    }

    // Read remote state
    let remote = read_remote_conversations(folder);

    // Build local lookup: (agent, id) → (updated_at, body)
    let mut local_map: HashMap<(String, String), (String, Vec<u8>)> = HashMap::new();
    for item in local_items {
        local_map.insert(
            (item.agent.clone(), item.id.clone()),
            (item.updated_at.clone(), item.body.clone()),
        );
    }

    let mut uploaded = 0usize;
    let mut downloaded = 0usize;
    let mut skipped = 0usize;
    let mut conflicts = 0usize;

    // Collect all unique conversation keys
    let mut all_keys: Vec<(String, String)> = Vec::new();
    for key in local_map.keys() {
        all_keys.push(key.clone());
    }
    for key in remote.keys() {
        if !local_map.contains_key(key) {
            all_keys.push(key.clone());
        }
    }

    for (agent, id) in &all_keys {
        let local_entry = local_map.get(&(agent.clone(), id.clone()));
        let remote_entry = remote.get(&(agent.clone(), id.clone()));

        match (local_entry, remote_entry) {
            // Only local → upload to sync folder
            (Some((local_ts, local_body)), None) => {
                let safe_name = id_to_filename(id);
                let file_path = conversations_dir.join(agent).join(format!("{safe_name}.json"));
                fs::write(&file_path, local_body)?;
                uploaded += 1;
                println!("↑ Uploaded {agent}/{id} (local_ts={local_ts})");
            }

            // Only remote → this is a conversation from another machine
            // We can't write to the agent's native storage, but we store it
            // in the sync folder so it's available for reading
            (None, Some((remote_ts, _remote_body))) => {
                // The conversation already exists in the sync folder.
                // We count it as "downloaded" — it will appear in the conversation
                // list when the app reads from the sync folder.
                downloaded += 1;
                println!("↓ Available from remote {agent}/{id} (remote_ts={remote_ts})");
            }

            // Both exist → compare timestamps
            (Some((local_ts, local_body)), Some((remote_ts, _remote_body))) => {
                let local_epoch = parse_timestamp(local_ts);
                let remote_epoch = parse_timestamp(remote_ts);

                if local_epoch > remote_epoch {
                    // Local is newer → upload
                    let safe_name = id_to_filename(id);
                    let file_path =
                        conversations_dir.join(agent).join(format!("{safe_name}.json"));
                    fs::write(&file_path, local_body)?;
                    uploaded += 1;
                    conflicts += 1;
                    println!(
                        "⟳ Conflict {agent}/{id}: local newer ({local_ts} > {remote_ts}), uploaded"
                    );
                } else if remote_epoch > local_epoch {
                    // Remote is newer → keep remote (it's already in the sync folder)
                    // The app should read from sync folder for the latest version
                    downloaded += 1;
                    conflicts += 1;
                    println!(
                        "⟳ Conflict {agent}/{id}: remote newer ({remote_ts} > {local_ts}), kept remote"
                    );
                } else {
                    // Same timestamp → skip
                    skipped += 1;
                }
            }

            (None, None) => unreachable!(),
        }
    }

    // Write manifest
    let manifest = serde_json::json!({
        "schema_version": 2,
        "app_version": env!("CARGO_PKG_VERSION"),
        "last_synced_at": chrono::Utc::now().to_rfc3339(),
        "sync_direction": "bidirectional",
        "uploaded": uploaded,
        "downloaded": downloaded,
        "skipped": skipped,
        "conflicts_resolved": conflicts,
        "total_local": local_items.len(),
        "total_remote": remote.len(),
    });
    fs::write(
        folder.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    Ok(SyncResult {
        uploaded,
        downloaded,
        skipped,
        conflicts_resolved: conflicts,
        folder_path: folder.to_string_lossy().to_string(),
    })
}

// ============================================================
// OneDrive sync status detection & safe auto-backup
// ============================================================

/// Check whether a sync folder is currently being written to by a cloud client
/// (OneDrive, Google Drive, Dropbox, etc.).
///
/// Detection heuristics:
/// 1. Any file with `.tmp` extension in the folder tree
/// 2. Any file starting with `~$` (Office lock files)
/// 3. Any file with `.partial` extension (OneDrive partial downloads)
/// 4. Any directory named `.odrive` or containing `.sync` files
/// 5. The folder itself was modified within the last `quiet_seconds`
///
/// Returns `true` if the folder appears to be in a "quiet" state (safe to sync).
pub fn is_folder_quiet(folder: &Path, quiet_seconds: u64) -> bool {
    if !folder.exists() {
        return true; // nothing to conflict with
    }

    // Check for lock/temp files
    if has_active_lock_files(folder) {
        return false;
    }

    // Check if the folder was recently modified
    if let Ok(metadata) = fs::metadata(folder) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed.as_secs() < quiet_seconds {
                    return false;
                }
            }
        }
    }

    true
}

/// Recursively check for lock/temp files that indicate active cloud sync.
fn has_active_lock_files(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden directories like .git, .DS_Store
        if name.starts_with('.') && path.is_dir() {
            // But check inside .sync-related dirs
            if name == ".odrive" || name == ".sync" || name == ".tmp.driveupload" {
                return true; // Active sync indicator
            }
            continue;
        }

        if path.is_dir() {
            if has_active_lock_files(&path) {
                return true;
            }
            continue;
        }

        let lower_name = name.to_lowercase();

        // OneDrive temp files
        if lower_name.ends_with(".tmp") || lower_name.ends_with(".partial") {
            return true;
        }

        // Office lock files
        if name.starts_with("~$") {
            return true;
        }

        // Google Drive temp files
        if lower_name.ends_with(".gdoc_tmp") || lower_name.contains(".crswap") {
            return true;
        }

        // Check if any file was written to within the last 3 seconds
        // (indicates active write operation)
        if let Ok(metadata) = fs::metadata(&path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    if elapsed.as_secs() < 3 {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Status of the cloud sync folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncReadiness {
    pub folder_exists: bool,
    pub is_quiet: bool,
    pub has_lock_files: bool,
    pub recommended_action: String, // "safe_to_sync" | "wait" | "folder_missing"
}

/// Check if it's safe to perform a sync right now.
pub fn check_cloud_readiness(folder: &Path, quiet_seconds: u64) -> CloudSyncReadiness {
    if !folder.exists() {
        return CloudSyncReadiness {
            folder_exists: false,
            is_quiet: true,
            has_lock_files: false,
            recommended_action: "folder_missing".to_string(),
        };
    }

    let has_locks = has_active_lock_files(folder);
    let quiet = is_folder_quiet(folder, quiet_seconds);

    let action = if has_locks || !quiet {
        "wait".to_string()
    } else {
        "safe_to_sync".to_string()
    };

    CloudSyncReadiness {
        folder_exists: true,
        is_quiet: quiet,
        has_lock_files: has_locks,
        recommended_action: action,
    }
}
