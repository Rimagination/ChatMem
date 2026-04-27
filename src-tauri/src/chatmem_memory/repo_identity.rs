use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const GLOBAL_LOCAL_HISTORY_ROOT: &str = "chatmem://local-history/global";

pub fn is_global_local_history_root(input: &str) -> bool {
    normalize_repo_root(input) == normalize_repo_root(GLOBAL_LOCAL_HISTORY_ROOT)
}

pub fn normalize_repo_root(input: &str) -> String {
    let mut normalized = input.trim().to_string();
    if let Some(stripped) = normalized.strip_prefix(r"\\?\UNC\") {
        normalized = format!("//{stripped}");
    } else if let Some(stripped) = normalized.strip_prefix(r"\\?\") {
        normalized = stripped.to_string();
    } else if let Some(stripped) = normalized.strip_prefix("//?/") {
        normalized = stripped.to_string();
    }

    normalized
        .trim_end_matches(['\\', '/'])
        .replace('\\', "/")
        .to_lowercase()
}

pub fn canonical_repo_root(input: &str) -> String {
    let normalized_fallback = normalize_repo_root(input);
    if input.trim().is_empty() {
        return normalized_fallback;
    }

    let input_path = PathBuf::from(input.trim());
    let mut cursor = if input_path.exists() {
        input_path.canonicalize().unwrap_or(input_path)
    } else {
        input_path
    };

    if cursor.is_file() {
        cursor.pop();
    }

    let mut current: Option<&Path> = Some(cursor.as_path());
    while let Some(path) = current {
        if path.join(".git").exists() {
            return normalize_repo_root(&path.to_string_lossy());
        }
        current = path.parent();
    }

    normalized_fallback
}

pub fn fingerprint_repo(repo_root: &str, git_remote: Option<&str>, branch: Option<&str>) -> String {
    let key = format!(
        "{}|{}|{}",
        normalize_repo_root(repo_root),
        git_remote.unwrap_or_default(),
        branch.unwrap_or_default()
    );

    Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_repo_root, fingerprint_repo, is_global_local_history_root, normalize_repo_root,
        GLOBAL_LOCAL_HISTORY_ROOT,
    };

    #[test]
    fn normalizes_windows_repo_root() {
        assert_eq!(
            normalize_repo_root(r"D:\VSP\agentswap-gui\"),
            "d:/vsp/agentswap-gui"
        );
    }

    #[test]
    fn fingerprint_is_stable_for_equivalent_repo_inputs() {
        let left = fingerprint_repo(
            r"D:\VSP\agentswap-gui\",
            Some("git@github.com:Rimagination/ChatMem.git"),
            Some("main"),
        );
        let right = fingerprint_repo(
            "d:/vsp/agentswap-gui",
            Some("git@github.com:Rimagination/ChatMem.git"),
            Some("main"),
        );

        assert_eq!(left, right);
    }

    #[test]
    fn canonical_repo_root_collapses_nested_paths_to_git_root() {
        let root =
            std::env::temp_dir().join(format!("chatmem-repo-root-test-{}", uuid::Uuid::new_v4()));
        let nested = root.join("src").join("chatmem_memory");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(&nested).unwrap();

        let canonical = canonical_repo_root(nested.to_str().unwrap());

        assert_eq!(canonical, normalize_repo_root(&root.to_string_lossy()));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn recognizes_global_local_history_root() {
        assert!(is_global_local_history_root(GLOBAL_LOCAL_HISTORY_ROOT));
        assert!(is_global_local_history_root(
            " ChatMem://Local-History/Global "
        ));
        assert!(!is_global_local_history_root("d:/vsp"));
    }
}
