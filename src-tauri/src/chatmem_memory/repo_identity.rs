use uuid::Uuid;

pub fn normalize_repo_root(input: &str) -> String {
    input
        .trim()
        .trim_end_matches(['\\', '/'])
        .replace('\\', "/")
        .to_lowercase()
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
    use super::{fingerprint_repo, normalize_repo_root};

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
}
