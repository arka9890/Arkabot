//! Per-agent workspace file store.
//!
//! Every agent owns a workspace directory under `<data_dir>/agents/<id>/workspace`
//! (mirroring OpenBot's per-agent local workspaces). Files are sandboxed: all
//! paths must resolve inside the agent's workspace root.

use openbot_core::error::{Error, Result};
use openbot_core::ids::AgentId;
use openbot_core::models::{FileEntry, FileTreeNode};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone)]
pub struct WorkspaceStore {
    data_dir: PathBuf,
}

impl WorkspaceStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn agent_root(&self, agent_id: &AgentId) -> PathBuf {
        self.data_dir
            .join("agents")
            .join(agent_id.as_str())
            .join("workspace")
    }

    fn safe_join(&self, agent_id: &AgentId, rel: &str) -> Result<PathBuf> {
        let root = self.agent_root(agent_id);

        // Lexically normalize first: reject absolute paths and collapse `..`
        // so the joined path can never climb above the workspace root. Popping
        // on an empty buffer is a no-op, which neutralizes leading `..`.
        let mut clean = PathBuf::new();
        for comp in Path::new(rel).components() {
            match comp {
                Component::RootDir | Component::Prefix(_) => {
                    return Err(Error::Workspace(format!("absolute path not allowed: {rel}")));
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    clean.pop();
                }
                Component::Normal(c) => clean.push(c),
            }
        }

        let joined = root.join(&clean);

        // Canonicalize the deepest existing ancestor to defend against
        // symlinks that resolve outside the workspace.
        let canonical_root = if root.exists() {
            fs::canonicalize(&root).unwrap_or_else(|_| root.clone())
        } else {
            root.clone()
        };
        let mut probe = joined.clone();
        while !probe.exists() {
            if !probe.pop() {
                break;
            }
        }
        let canonical_probe = fs::canonicalize(&probe).unwrap_or_else(|_| probe.clone());
        let inside = canonical_probe == canonical_root
            || canonical_probe.starts_with(&canonical_root)
            || !canonical_root.exists();
        if inside {
            Ok(joined)
        } else {
            Err(Error::Workspace(format!("path escapes workspace: {rel}")))
        }
    }

    pub fn ensure_root(&self, agent_id: &AgentId) -> Result<()> {
        fs::create_dir_all(self.agent_root(agent_id))?;
        Ok(())
    }

    pub fn list(&self, agent_id: &AgentId, dir: &str) -> Result<Vec<FileEntry>> {
        let path = self.safe_join(agent_id, dir)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&path).map_err(|e| Error::Workspace(format!("read_dir {dir}: {e}")))? {
            let entry = entry.map_err(|e| Error::Workspace(format!("entry: {e}")))?;
            let meta = entry
                .metadata()
                .map_err(|e| Error::Workspace(format!("metadata: {e}")))?;
            let rel = entry
                .path()
                .strip_prefix(self.agent_root(agent_id))
                .map_err(|_| Error::Workspace("strip".into()))?
                .to_string_lossy()
                .replace('\\', "/");
            entries.push(FileEntry {
                path: rel,
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: meta.is_dir(),
                size: meta.len(),
                updated_at: chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)),
            });
        }
        entries.sort_by(|a, b| (b.is_dir, a.name.as_str()).cmp(&(a.is_dir, b.name.as_str())));
        Ok(entries)
    }

    pub fn tree(&self, agent_id: &AgentId, dir: &str) -> Result<FileTreeNode> {
        fn build(store: &WorkspaceStore, agent_id: &AgentId, path: &Path) -> Result<FileTreeNode> {
            let entries = store.list(agent_id, &path.to_string_lossy().replace('\\', "/"))?;
            let node = FileTreeNode {
                path: path.to_string_lossy().replace('\\', "/"),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "/".into()),
                is_dir: true,
                children: Vec::new(),
            };
            let mut children = Vec::new();
            for entry in &entries {
                if entry.is_dir {
                    children.push(build(store, agent_id, &Path::new(&entry.path))?);
                }
            }
            for entry in &entries {
                if !entry.is_dir {
                    children.push(FileTreeNode {
                        path: entry.path.clone(),
                        name: entry.name.clone(),
                        is_dir: false,
                        children: Vec::new(),
                    });
                }
            }
            Ok(FileTreeNode {
                path: node.path,
                name: node.name,
                is_dir: true,
                children,
            })
        }
        let root = self.safe_join(agent_id, dir)?;
        build(self, agent_id, &root)
    }

    pub fn read(&self, agent_id: &AgentId, rel: &str) -> Result<String> {
        let path = self.safe_join(agent_id, rel)?;
        if !path.is_file() {
            return Err(Error::Workspace(format!("not a file: {rel}")));
        }
        fs::read_to_string(&path).map_err(|e| Error::Workspace(format!("read {rel}: {e}")))
    }

    pub fn write(&self, agent_id: &AgentId, rel: &str, content: &str) -> Result<FileEntry> {
        let path = self.safe_join(agent_id, rel)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        let meta = fs::metadata(&path)?;
        Ok(FileEntry {
            path: rel.to_string(),
            name: Path::new(rel)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            is_dir: false,
            size: meta.len(),
            updated_at: chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)),
        })
    }

    pub fn create_dir(&self, agent_id: &AgentId, rel: &str) -> Result<()> {
        let path = self.safe_join(agent_id, rel)?;
        fs::create_dir_all(&path)?;
        Ok(())
    }

    pub fn delete(&self, agent_id: &AgentId, rel: &str) -> Result<()> {
        let path = self.safe_join(agent_id, rel)?;
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn attachments_dir(&self) -> PathBuf {
        self.data_dir.join("attachments")
    }
}
