use std::fs;
use std::path::{Path, PathBuf};

use crate::io_error;

/// Directories a tree walk never enters, matching the Python search tools.
pub(crate) const SKIPPED_DIRS: [&str; 5] =
    [".git", "target", "node_modules", ".venv", "__pycache__"];

/// Every regular file below `root`, sorted, with skipped directories and
/// symlinks omitted.
pub(crate) fn files(root: &Path) -> Result<Vec<PathBuf>, crate::ToolError> {
    let mut files = Vec::new();
    visit(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), crate::ToolError> {
    let entries = fs::read_dir(dir).map_err(|source| io_error(dir, source))?;
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| io_error(dir, source))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIPPED_DIRS.contains(&name.as_ref()) {
                continue;
            }
            visit(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
