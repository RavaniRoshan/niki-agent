use std::path::{Component, Path, PathBuf};

use crate::{ToolError, io_error};

/// The workspace a tool may touch.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Root of the workspace. Paths handed to a tool are joined to it.
    pub root: PathBuf,
}

impl ToolContext {
    /// Canonicalise and validate a workspace root.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, ToolError> {
        let root = root.into();
        let canonical = root
            .canonicalize()
            .map_err(|source| io_error(&root, source))?;
        Ok(Self { root: canonical })
    }

    /// Resolve a tool path argument against the root, refusing any result that
    /// leaves the workspace.
    ///
    /// The nearest existing ancestor is canonicalised (so symlinks are followed
    /// before the boundary check), remaining components are normalised
    /// lexically, and a final symlink at the leaf is refused when it points
    /// outside the root or is dangling.
    pub fn resolve(&self, raw: &str) -> Result<PathBuf, ToolError> {
        let root = self
            .root
            .canonicalize()
            .map_err(|source| io_error(&self.root, source))?;
        let joined = root.join(raw);

        let mut existing = joined.as_path();
        let ancestor = loop {
            match std::fs::metadata(existing) {
                Ok(_) => break existing,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    existing = existing.parent().ok_or_else(|| self.outside(&root, raw))?;
                }
                Err(source) => return Err(io_error(existing, source)),
            }
        };

        let mut resolved = ancestor
            .canonicalize()
            .map_err(|source| io_error(ancestor, source))?;
        // INFALLIBLE: `ancestor` is an ancestor of `joined`, so stripping it succeeds.
        let tail = joined.strip_prefix(ancestor).unwrap_or(&joined);
        for component in tail.components() {
            match component {
                Component::Normal(part) => resolved.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if resolved == root || !resolved.pop() {
                        return Err(self.outside(&root, raw));
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(self.outside(&root, raw));
                }
            }
            if !resolved.starts_with(&root) {
                return Err(self.outside(&root, raw));
            }
        }

        if !resolved.starts_with(&root) {
            return Err(self.outside(&root, raw));
        }

        if let Ok(metadata) = std::fs::symlink_metadata(&resolved) {
            if metadata.file_type().is_symlink() {
                let target = resolved
                    .canonicalize()
                    .map_err(|_| self.outside(&root, raw))?;
                if !target.starts_with(&root) {
                    return Err(self.outside(&root, raw));
                }
                resolved = target;
            }
        }

        Ok(resolved)
    }

    /// Render a resolved path relative to the root with `/` separators, for
    /// tool output that a model reads on any platform.
    pub fn relative(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".to_owned(),
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => path.to_string_lossy().replace('\\', "/"),
        }
    }

    fn outside(&self, root: &Path, raw: &str) -> ToolError {
        ToolError::OutsideRoot {
            root: root.display().to_string(),
            path: raw.to_owned(),
        }
    }
}
