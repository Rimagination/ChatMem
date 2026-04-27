use std::path::Path;

use anyhow::{Context, Result};

pub fn move_path_to_trash(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    trash::delete(path)
        .with_context(|| format!("Failed to move file to system Trash: {}", path.display()))
}
