use crate::error::{KnackError, Result};
use crate::registry::{self, Agent};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(rename = "skillsRoot")]
    pub skills_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillSpec {
    pub name: String,
    pub source: Option<String>,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    agents: Vec<Agent>,
    #[serde(default)]
    skills: Vec<SkillSpec>,
}

#[derive(Debug, Clone)]
pub struct ConfigState {
    pub path: PathBuf,
    pub exists: bool,
    pub settings: Settings,
    pub agents: Vec<Agent>,
    pub skills: Vec<SkillSpec>,
    pub raw_text: Option<String>,
}

pub fn load() -> Result<ConfigState> {
    let defaults = registry::default_agents();
    let path = config_path()?;
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                return Ok(ConfigState {
                    path,
                    exists: false,
                    settings: Settings::default(),
                    agents: defaults,
                    skills: Vec::new(),
                    raw_text: None,
                });
            }
            return Err(err.into());
        }
    };
    let raw: RawConfig = json5::from_str(&data)?;
    let agents = if raw.agents.is_empty() {
        defaults.clone()
    } else {
        registry::merge_agents(&defaults, &raw.agents)
    };

    Ok(ConfigState {
        path,
        exists: true,
        settings: raw.settings,
        agents,
        skills: raw.skills,
        raw_text: Some(data),
    })
}

pub fn save(state: &ConfigState) -> Result<()> {
    if let Some(parent) = state.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = RawConfig {
        settings: state.settings.clone(),
        agents: state.agents.clone(),
        skills: state.skills.clone(),
    };
    let json = serde_json::to_string_pretty(&raw)?;
    let output = if let Some(raw_text) = state.raw_text.as_deref() {
        let header = extract_jsonc_header(raw_text);
        if header.is_empty() {
            json
        } else {
            let mut out = String::new();
            out.push_str(header);
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&json);
            out
        }
    } else {
        json
    };
    std::fs::write(&state.path, output)?;
    Ok(())
}

pub fn config_path() -> Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| KnackError::msg("unable to resolve home directory"))?;
    let primary = if cfg!(windows) {
        if let Some(config_dir) = dirs::config_dir() {
            if !config_dir.as_os_str().is_empty() {
                config_dir.join("knack").join("config.jsonc")
            } else {
                home.join(".config").join("knack").join("config.jsonc")
            }
        } else {
            home.join(".config").join("knack").join("config.jsonc")
        }
    } else {
        home.join(".config").join("knack").join("config.jsonc")
    };
    Ok(primary)
}

fn extract_jsonc_header(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx + 1 < bytes.len() && bytes[idx] == b'/' && bytes[idx + 1] == b'/' {
            idx += 2;
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        if idx + 1 < bytes.len() && bytes[idx] == b'/' && bytes[idx + 1] == b'*' {
            idx += 2;
            while idx + 1 < bytes.len()
                && !(bytes[idx] == b'*' && bytes[idx + 1] == b'/')
            {
                idx += 1;
            }
            if idx + 1 < bytes.len() {
                idx += 2;
            }
            continue;
        }
        break;
    }
    &raw[..idx]
}
