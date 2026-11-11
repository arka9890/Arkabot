//! Local skill library.
//!
//! Mirrors OpenBot's local skills: one shared library of `SKILL.md` folders
//! under `<data_dir>/skills/<id>/`, and a per-agent installed copy under
//! `<agent workspace>/.agents/skills/<id>/`. Revising a skill keeps the
//! previous revision and requires the version the caller read, so a
//! concurrent edit is rejected instead of overwritten.

use openbot_core::error::{Error, Result};
use openbot_core::ids::AgentId;
use openbot_core::models::Skill;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SKILL_FILE: &str = "SKILL.md";

/// Frontmatter of a `SKILL.md`. Only the three fields OpenBot's local skills
/// need are read; everything else in the body is kept verbatim.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
}

/// A `SKILL.md` split into frontmatter and body.
#[derive(Debug, Clone, Default)]
pub struct SkillFile {
    pub frontmatter: SkillFrontmatter,
    pub body: String,
}

impl SkillFile {
    /// Split a `SKILL.md` into its frontmatter and body.
    ///
    /// A file without a leading `---` block has no frontmatter; the whole
    /// text is the body.
    pub fn parse(text: &str) -> Self {
        let all: Vec<&str> = text.lines().collect();
        if all.is_empty() || all[0].trim() != "---" {
            return Self {
                frontmatter: SkillFrontmatter::default(),
                body: text.to_string(),
            };
        }
        let mut fm = SkillFrontmatter::default();
        let mut body_start = 0usize;
        for (i, line) in all.iter().enumerate().skip(1) {
            if line.trim() == "---" {
                body_start = i + 1;
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                match key {
                    "name" => fm.name = value.to_string(),
                    "description" => fm.description = value.to_string(),
                    "version" => fm.version = value.to_string(),
                    _ => {}
                }
            }
        }
        if body_start == 0 {
            // No closing fence — treat the file as body-only.
            return Self {
                frontmatter: SkillFrontmatter::default(),
                body: text.to_string(),
            };
        }
        let body = all[body_start..].join("\n");
        let body = body.trim_start_matches('\n').to_string();
        Self { frontmatter: fm, body }
    }

    /// Rebuild the file text from frontmatter and body.
    pub fn render(frontmatter: &SkillFrontmatter, body: &str) -> String {
        format!(
            "---\nname: {}\ndescription: {}\nversion: {}\n---\n\n{}",
            frontmatter.name, frontmatter.description, frontmatter.version, body
        )
    }
}

/// The shared skill library plus the per-agent installed copies.
#[derive(Clone)]
pub struct SkillLibrary {
    data_dir: PathBuf,
}

impl SkillLibrary {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    /// `<data_dir>/skills` — the shared library.
    pub fn library_root(&self) -> PathBuf {
        self.data_dir.join("skills")
    }

    /// `<data_dir>/skills/<id>` — one library entry.
    pub fn library_dir(&self, id: &str) -> Result<PathBuf> {
        let id = validate_id(id)?;
        Ok(self.library_root().join(id))
    }

    /// `<agent workspace>/.agents/skills` — an agent's installed copies.
    pub fn installed_root(&self, agent_id: &AgentId) -> PathBuf {
        self.data_dir
            .join("agents")
            .join(agent_id.as_str())
            .join("workspace")
            .join(".agents")
            .join("skills")
    }

    /// Every skill in the library.
    pub fn list(&self) -> Result<Vec<Skill>> {
        let root = self.library_root();
        let mut out = Vec::new();
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(Error::Storage(format!("read skills dir: {e}"))),
        };
        for entry in entries {
            let entry = entry.map_err(|e| Error::Storage(format!("skill entry: {e}")))?;
            if !entry.path().is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            match self.read(&id) {
                Ok(skill) => out.push(skill),
                // A folder without a SKILL.md is not a skill; skip it.
                Err(_) => continue,
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    /// Read one library skill.
    pub fn read(&self, id: &str) -> Result<Skill> {
        let dir = self.library_dir(id)?;
        let path = dir.join(SKILL_FILE);
        let text = fs::read_to_string(&path)
            .map_err(|e| Error::NotFound(format!("skill {id}: {e}")))?;
        let parsed = SkillFile::parse(&text);
        let meta = fs::metadata(&path).ok();
        Ok(Skill {
            id: id.to_string(),
            name: if parsed.frontmatter.name.is_empty() {
                id.to_string()
            } else {
                parsed.frontmatter.name.clone()
            },
            description: parsed.frontmatter.description.clone(),
            version: parsed.frontmatter.version.clone(),
            body: parsed.body.clone(),
            installed: false,
            updated_at: meta
                .and_then(|m| m.modified().ok())
                .map(chrono::DateTime::<chrono::Utc>::from)
                .unwrap_or_else(|| chrono::Utc::now()),
        })
    }

    /// Create or revise a library skill.
    ///
    /// `expected_version` is the version the caller read. It must match the
    /// stored version (or the skill must not exist yet), otherwise the
    /// revise is rejected. The previous revision is kept under `versions/`.
    pub fn save(&self, id: &str, content: &str, expected_version: Option<&str>) -> Result<Skill> {
        let dir = self.library_dir(id)?;
        let path = dir.join(SKILL_FILE);
        let existing = if path.is_file() {
            Some(self.read(id)?)
        } else {
            None
        };

        if let Some(current) = &existing {
            if let Some(expected) = expected_version {
                if !expected.is_empty() && expected != current.version {
                    return Err(Error::AlreadyExists(format!(
                        "skill {id} changed: version {} != {expected}",
                        current.version
                    )));
                }
            }
        }

        let mut parsed = SkillFile::parse(content);
        if parsed.frontmatter.name.is_empty() {
            parsed.frontmatter.name = id.to_string();
        }
        if parsed.frontmatter.version.is_empty() {
            parsed.frontmatter.version = match &existing {
                Some(current) if !current.version.is_empty() => current.version.clone(),
                _ => "1".to_string(),
            };
        }

        // Keep the previous revision so an agent can install an exact one.
        if let Some(current) = &existing {
            if !current.version.is_empty() && current.version != parsed.frontmatter.version {
                let versions = dir.join("versions");
                fs::create_dir_all(&versions)?;
                let backup = versions.join(format!("{}.md", sanitize_version(&current.version)));
                if !backup.exists() {
                    fs::write(&backup, content_of(&path)?)?;
                }
            }
        }

        fs::create_dir_all(&dir)?;
        fs::write(&path, SkillFile::render(&parsed.frontmatter, &parsed.body))?;

        let mut skill = self.read(id)?;
        skill.body = parsed.body;
        Ok(skill)
    }

    /// Remove a skill from the library.
    pub fn delete(&self, id: &str) -> Result<()> {
        let dir = self.library_dir(id)?;
        if !dir.exists() {
            return Err(Error::NotFound(format!("skill {id}")));
        }
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    /// Install a library skill into an agent's workspace.
    pub fn install(&self, agent_id: &AgentId, id: &str) -> Result<Skill> {
        let skill = self.read(id)?;
        let target = self.installed_root(agent_id).join(&skill.id);
        fs::create_dir_all(&target)?;
        fs::write(
            target.join(SKILL_FILE),
            SkillFile::render(
                &SkillFrontmatter {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    version: skill.version.clone(),
                },
                &skill.body,
            ),
        )?;
        Ok(Skill { installed: true, ..skill })
    }

    /// Remove an installed skill from an agent's workspace.
    pub fn uninstall(&self, agent_id: &AgentId, id: &str) -> Result<()> {
        let id = validate_id(id)?;
        let target = self.installed_root(agent_id).join(id);
        if !target.exists() {
            return Err(Error::NotFound(format!("skill {id} not installed")));
        }
        fs::remove_dir_all(&target)?;
        Ok(())
    }

    /// The skills installed for one agent.
    pub fn installed(&self, agent_id: &AgentId) -> Result<Vec<Skill>> {
        let root = self.installed_root(agent_id);
        let mut out = Vec::new();
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(Error::Storage(format!("read installed skills: {e}"))),
        };
        for entry in entries {
            let entry = entry.map_err(|e| Error::Storage(format!("skill entry: {e}")))?;
            let path = entry.path().join(SKILL_FILE);
            if !path.is_file() {
                continue;
            }
            let text = fs::read_to_string(&path)
                .map_err(|e| Error::Storage(format!("read installed skill: {e}")))?;
            let parsed = SkillFile::parse(&text);
            let id = entry.file_name().to_string_lossy().into_owned();
            out.push(Skill {
                name: if parsed.frontmatter.name.is_empty() {
                    id.clone()
                } else {
                    parsed.frontmatter.name.clone()
                },
                description: parsed.frontmatter.description.clone(),
                version: parsed.frontmatter.version.clone(),
                body: parsed.body.clone(),
                installed: true,
                id,
                updated_at: fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .map(chrono::DateTime::<chrono::Utc>::from)
                    .unwrap_or_else(|_| chrono::Utc::now()),
            });
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    /// The library skills marked with whether they are installed for `agent_id`.
    pub fn list_for_agent(&self, agent_id: &AgentId) -> Result<Vec<Skill>> {
        let installed: Vec<String> = self
            .installed(agent_id)?
            .into_iter()
            .map(|s| s.id)
            .collect();
        Ok(self
            .list()?
            .into_iter()
            .map(|mut s| {
                s.installed = installed.contains(&s.id);
                s
            })
            .collect())
    }
}

fn content_of(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|e| Error::Storage(format!("read {}: {e}", path.display())))
}

/// Skill ids become folder names, so they must not escape the library.
fn validate_id(id: &str) -> Result<&str> {
    if id.is_empty() {
        return Err(Error::InvalidId("skill id is empty".into()));
    }
    let ok = id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.');
    if !ok || id.starts_with('.') || id.contains("..") {
        return Err(Error::InvalidId(format!("skill id: {id}")));
    }
    Ok(id)
}

/// Version strings become file names inside `versions/`.
fn sanitize_version(version: &str) -> String {
    let cleaned: String = version
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "1".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let text = "---\nname: notes\ndescription: taking notes\nversion: 2\n---\n\nDo the thing.\n";
        let parsed = SkillFile::parse(text);
        assert_eq!(parsed.frontmatter.name, "notes");
        assert_eq!(parsed.frontmatter.description, "taking notes");
        assert_eq!(parsed.frontmatter.version, "2");
        assert_eq!(parsed.body.trim(), "Do the thing.");
    }

    #[test]
    fn file_without_frontmatter_is_all_body() {
        let parsed = SkillFile::parse("just text");
        assert!(parsed.frontmatter.name.is_empty());
        assert_eq!(parsed.body, "just text");
    }

    #[test]
    fn render_round_trips() {
        let fm = SkillFrontmatter {
            name: "a".into(),
            description: "b".into(),
            version: "3".into(),
        };
        let text = SkillFile::render(&fm, "body");
        let parsed = SkillFile::parse(&text);
        assert_eq!(parsed.frontmatter.name, "a");
        assert_eq!(parsed.frontmatter.version, "3");
        assert_eq!(parsed.body.trim(), "body");
    }

    #[test]
    fn rejects_path_escaping_ids() {
        assert!(validate_id("../evil").is_err());
        assert!(validate_id("ok-name_1.2").is_ok());
    }

    /// A scratch directory that cleans itself up.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "arkabot-skills-{tag}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn saves_installs_and_revises_a_skill() {
        let tmp = TempDir::new("save");
        let lib = SkillLibrary::new(&tmp.0);
        let agent = AgentId::new();

        let content = "---\nname: notes\ndescription: taking notes\nversion: 1\n---\n\nTake notes.";
        let skill = lib.save("notes", content, None).unwrap();
        assert_eq!(skill.name, "notes");
        assert_eq!(skill.version, "1");

        // The library lists it; the agent has none installed yet.
        assert_eq!(lib.list().unwrap().len(), 1);
        assert!(lib.installed(&agent).unwrap().is_empty());

        // Installing copies it into the agent workspace.
        lib.install(&agent, "notes").unwrap();
        let installed = lib.installed(&agent).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, "notes");
        assert!(lib.list_for_agent(&agent).unwrap()[0].installed);

        // A revise that quotes a stale version is rejected.
        let stale = lib
            .save(
                "notes",
                "---\nname: notes\ndescription: d\nversion: 2\n---\n\nNew body.",
                Some("0"),
            )
            .unwrap_err();
        assert!(matches!(stale, Error::AlreadyExists(_)));

        // A revise that quotes the current version goes through.
        let revised = lib
            .save(
                "notes",
                "---\nname: notes\ndescription: d\nversion: 2\n---\n\nNew body.",
                Some("1"),
            )
            .unwrap();
        assert_eq!(revised.version, "2");
        // The previous revision is kept.
        assert!(tmp.0.join("skills/notes/versions/1.md").is_file());

        // Uninstalling removes only the agent's copy.
        lib.uninstall(&agent, "notes").unwrap();
        assert!(lib.installed(&agent).unwrap().is_empty());
        assert_eq!(lib.list().unwrap().len(), 1);
    }
}
