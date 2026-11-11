//! Agent import and export.
//!
//! An agent travels as one zip file: `agent.json` holds the metadata,
//! `AGENTS.md` the instructions, and `workspace/` the files. Importing
//! rebuilds the agent from that file, so an agent can be shared between
//! devices without carrying the whole workspace database.

use openbot_core::error::{Error, Result};
use openbot_core::ids::AgentId;
use openbot_core::models::{Agent, BusyMessageMode, Capability, ProviderKind};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

/// Metadata stored in `agent.json` inside the archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArchive {
    pub name: String,
    pub description: String,
    pub avatar: String,
    pub avatar_color: String,
    pub instructions: String,
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub max_tool_iterations: u32,
    pub compaction_messages: u32,
    pub busy_mode: String,
    /// Named browser profile; a shared archive keeps the name so the
    /// importing device can map it to a local profile.
    pub browser_profile: String,
    pub capabilities: Vec<String>,
}

impl AgentArchive {
    /// Build the archive metadata from a live agent.
    pub fn from_agent(agent: &Agent) -> Self {
        Self {
            name: agent.name.clone(),
            description: agent.description.clone(),
            avatar: agent.avatar.clone(),
            avatar_color: agent.avatar_color.clone(),
            instructions: agent.instructions.clone(),
            provider: agent.provider.as_str().to_string(),
            model: agent.model.clone(),
            temperature: agent.temperature,
            max_tool_iterations: agent.max_tool_iterations,
            compaction_messages: agent.compaction_messages,
            busy_mode: agent.busy_mode.as_str().to_string(),
            browser_profile: agent.browser_profile.clone(),
            capabilities: agent
                .capabilities
                .iter()
                .map(|c| capability_str(*c))
                .collect(),
        }
    }

    /// The agent this archive describes, with a fresh id and no workspace.
    pub fn to_agent(&self) -> Agent {
        let now = chrono::Utc::now();
        Agent {
            id: AgentId::new(),
            name: self.name.clone(),
            description: self.description.clone(),
            avatar: self.avatar.clone(),
            avatar_color: self.avatar_color.clone(),
            instructions: self.instructions.clone(),
            provider: self.provider.parse::<ProviderKind>().unwrap_or_default(),
            model: self.model.clone(),
            temperature: self.temperature,
            max_tool_iterations: self.max_tool_iterations,
            compaction_messages: self.compaction_messages,
            busy_mode: self.busy_mode.parse::<BusyMessageMode>().unwrap_or_default(),
            phase: openbot_core::models::AgentPhase::Idle,
            workspace_root: String::new(),
            browser_profile: self.browser_profile.clone(),
            capabilities: self
                .capabilities
                .iter()
                .filter_map(|c| capability_from_str(c))
                .collect(),
            created_at: now,
            updated_at: now,
        }
    }
}

fn capability_str(cap: Capability) -> String {
    match cap {
        Capability::Chat => "chat",
        Capability::Files => "files",
        Capability::Browser => "browser",
        Capability::TaskQueue => "task_queue",
        Capability::Notifications => "notifications",
    }
    .to_string()
}

fn capability_from_str(s: &str) -> Option<Capability> {
    match s {
        "chat" => Some(Capability::Chat),
        "files" => Some(Capability::Files),
        "browser" => Some(Capability::Browser),
        "task_queue" => Some(Capability::TaskQueue),
        "notifications" => Some(Capability::Notifications),
        _ => None,
    }
}

/// Write an agent and its workspace into `zip_path`.
///
/// `workspace_root` is the agent's workspace directory; it is added under
/// `workspace/` in the archive.
pub fn export_agent_to_zip(agent: &Agent, workspace_root: &Path, zip_path: &Path) -> Result<()> {
    if let Some(parent) = zip_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(zip_path)
        .map_err(|e| Error::Io(format!("create {}: {e}", zip_path.display())))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    let archive = AgentArchive::from_agent(agent);
    let meta = serde_json::to_vec_pretty(&archive)
        .map_err(|e| Error::Json(format!("agent json: {e}")))?;
    zip.start_file("agent.json", options)
        .map_err(|e| Error::Io(format!("zip agent.json: {e}")))?;
    zip.write_all(&meta)?;

    zip.start_file("AGENTS.md", options)
        .map_err(|e| Error::Io(format!("zip AGENTS.md: {e}")))?;
    zip.write_all(agent.instructions.as_bytes())?;

    if workspace_root.is_dir() {
        add_dir(&mut zip, workspace_root, Path::new("workspace"), options)?;
    }
    zip.finish()
        .map_err(|e| Error::Io(format!("finish zip: {e}")))?;
    Ok(())
}

fn add_dir(
    zip: &mut ZipWriter<fs::File>,
    dir: &Path,
    prefix: &Path,
    options: SimpleFileOptions,
) -> Result<()> {
    let entries =
        fs::read_dir(dir).map_err(|e| Error::Io(format!("read {}: {e}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::Io(format!("entry: {e}")))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            // Skip dotfiles such as `.agents` — installed skills are not part
            // of the shared agent.
            continue;
        }
        let target = prefix.join(&name);
        if path.is_dir() {
            zip.add_directory(
                target.to_string_lossy().replace('\\', "/"),
                options,
            )
            .map_err(|e| Error::Io(format!("zip dir: {e}")))?;
            add_dir(zip, &path, &target, options)?;
        } else {
            let data = fs::read(&path).map_err(|e| Error::Io(format!("read file: {e}")))?;
            zip.start_file(target.to_string_lossy().replace('\\', "/"), options)
                .map_err(|e| Error::Io(format!("zip file: {e}")))?;
            zip.write_all(&data)?;
        }
    }
    Ok(())
}

/// What an archive carries.
pub struct ImportedAgent {
    pub agent: Agent,
    /// Files to write into the agent's workspace, keyed by relative path.
    pub files: Vec<(String, Vec<u8>)>,
}

/// Read an agent archive without touching the database.
pub fn read_agent_zip(zip_path: &Path) -> Result<ImportedAgent> {
    let file = fs::File::open(zip_path)
        .map_err(|e| Error::NotFound(format!("open {}: {e}", zip_path.display())))?;
    let mut archive = ZipArchive::new(file).map_err(|e| Error::Io(format!("zip open: {e}")))?;

    let mut meta_text = String::new();
    archive
        .by_name("agent.json")
        .map_err(|e| Error::NotFound(format!("agent.json: {e}")))?
        .read_to_string(&mut meta_text)
        .map_err(|e| Error::Io(format!("read agent.json: {e}")))?;
    let archive_meta: AgentArchive = serde_json::from_str(&meta_text)
        .map_err(|e| Error::Json(format!("agent.json: {e}")))?;

    let mut instructions = archive_meta.instructions.clone();
    if let Ok(mut file) = archive.by_name("AGENTS.md") {
        let mut text = String::new();
        file.read_to_string(&mut text)
            .map_err(|e| Error::Io(format!("read AGENTS.md: {e}")))?;
        instructions = text;
    }

    let mut agent = archive_meta.to_agent();
    agent.instructions = instructions;

    let mut files = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| Error::Io(format!("zip entry: {e}")))?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().to_string();
        let Some(rel) = name.strip_prefix("workspace/") else {
            continue;
        };
        if rel.is_empty() || rel.starts_with('/') || rel.contains("..") {
            continue;
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| Error::Io(format!("read {name}: {e}")))?;
        files.push((rel.to_string(), data));
    }

    Ok(ImportedAgent { agent, files })
}

/// A file name safe for the exported archive.
pub fn archive_file_name(agent_name: &str) -> String {
    let cleaned: String = agent_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "agent".into()
    } else {
        cleaned
    }
}

/// Where an exported archive for `agent` goes inside `target_dir`.
pub fn archive_path(target_dir: &Path, agent_name: &str) -> PathBuf {
    target_dir.join(format!("{}.zip", archive_file_name(agent_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_agent() -> Agent {
        let now = chrono::Utc::now();
        Agent {
            id: AgentId::new(),
            name: "Coder Bot".into(),
            description: "writes code".into(),
            avatar: "💻".into(),
            avatar_color: "#9ece6a".into(),
            instructions: "You are a coder.".into(),
            provider: ProviderKind::LmStudio,
            model: "qwen".into(),
            temperature: 0.3,
            max_tool_iterations: 4,
            compaction_messages: 20,
            busy_mode: BusyMessageMode::Queue,
            phase: openbot_core::models::AgentPhase::Idle,
            workspace_root: String::new(),
            browser_profile: String::new(),
            capabilities: vec![Capability::Chat, Capability::Files],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn exports_and_imports_an_agent() {
        let dir = std::env::temp_dir().join(format!(
            "arkabot-export-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        let workspace = dir.join("workspace");
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(workspace.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(workspace.join("README.md"), "# hi").unwrap();

        let agent = sample_agent();
        let zip_path = dir.join("out.zip");
        export_agent_to_zip(&agent, &workspace, &zip_path).unwrap();

        let imported = read_agent_zip(&zip_path).unwrap();
        assert_eq!(imported.agent.name, "Coder Bot");
        assert_eq!(imported.agent.instructions, "You are a coder.");
        assert_eq!(imported.agent.provider, ProviderKind::LmStudio);
        assert_ne!(imported.agent.id, agent.id, "import must get a new id");
        let names: Vec<&str> = imported.files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"src/main.rs"));
        assert!(names.contains(&"README.md"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_name_is_sanitized() {
        assert_eq!(archive_file_name("Coder Bot"), "Coder-Bot");
        assert_eq!(archive_file_name("///"), "agent");
    }
}
