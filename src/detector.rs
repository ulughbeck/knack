use crate::registry::Agent;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Project,
    Global,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::Global => "global",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub agent: Agent,
    pub scope: Scope,
    pub root: PathBuf,
    pub skills_dir: PathBuf,
}

pub fn detect_targets(agents: &[Agent], cwd: &Path) -> (Vec<Target>, Vec<String>) {
    let warnings = Vec::new();
    let mut targets = Vec::new();

    for agent in agents {
        if agent.id.is_empty() {
            continue;
        }
        if let Some(project_root) = resolve_project_config(agent, cwd) {
            if exists_dir(&project_root) {
                targets.push(Target {
                    agent: agent.clone(),
                    scope: Scope::Project,
                    skills_dir: join_skills_dir(&project_root, &agent.skills_dir),
                    root: project_root,
                });
            }
        }
        if let Some(global_root) = resolve_global_config(agent) {
            if exists_dir(&global_root) {
                targets.push(Target {
                    agent: agent.clone(),
                    scope: Scope::Global,
                    skills_dir: join_skills_dir(&global_root, &agent.skills_dir),
                    root: global_root,
                });
            }
        }
    }

    (targets, warnings)
}

pub fn default_targets(targets: &[Target]) -> Vec<Target> {
    let mut by_agent = std::collections::HashMap::new();
    let mut order = Vec::new();

    for target in targets {
        if target.agent.id.is_empty() {
            continue;
        }
        if !by_agent.contains_key(&target.agent.id) {
            order.push(target.agent.id.clone());
        }
        if target.scope == Scope::Project {
            by_agent.insert(target.agent.id.clone(), target.clone());
            continue;
        }
        if !by_agent.contains_key(&target.agent.id) {
            by_agent.insert(target.agent.id.clone(), target.clone());
        }
    }

    let mut defaults = Vec::with_capacity(by_agent.len());
    for id in order {
        if let Some(target) = by_agent.get(&id) {
            defaults.push(target.clone());
        }
    }
    defaults
}

pub fn filter_scope(targets: &[Target], scope: Scope) -> Vec<Target> {
    targets
        .iter()
        .cloned()
        .filter(|t| t.scope == scope)
        .collect()
}

pub fn global_skills_dir(agent: &Agent) -> Option<PathBuf> {
    resolve_global_config(agent).map(|root| join_skills_dir(&root, &agent.skills_dir))
}

fn resolve_project_config(agent: &Agent, cwd: &Path) -> Option<PathBuf> {
    if agent.project_config.is_empty() {
        return None;
    }
    let path = Path::new(&agent.project_config);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    Some(cwd.join(path))
}

fn resolve_global_config(agent: &Agent) -> Option<PathBuf> {
    if agent.global_config.is_empty() {
        return None;
    }
    let path = Path::new(&agent.global_config);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    let clean = path_clean(&agent.global_config);
    if is_dot_config_path(&clean) && cfg!(windows) {
        if let Some(config_dir) = dirs::config_dir() {
            let mut trimmed = clean.strip_prefix(".config").unwrap_or("");
            trimmed = trimmed
                .strip_prefix(std::path::MAIN_SEPARATOR)
                .unwrap_or(trimmed);
            if trimmed.is_empty() {
                return Some(config_dir);
            }
            return Some(config_dir.join(trimmed));
        }
    }
    if let Some(home) = dirs::home_dir() {
        return Some(home.join(clean));
    }
    None
}

fn is_dot_config_path(path: &str) -> bool {
    if path == ".config" {
        return true;
    }
    let prefix = format!(".config{}", std::path::MAIN_SEPARATOR);
    path.starts_with(&prefix)
}

fn path_clean(input: &str) -> String {
    let mut components = Path::new(input).components().peekable();
    let mut stack: Vec<&std::ffi::OsStr> = Vec::new();
    while let Some(comp) = components.next() {
        use std::path::Component;
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                stack.pop();
            }
            Component::Normal(part) => stack.push(part),
            Component::RootDir | Component::Prefix(_) => {
                stack.clear();
            }
        }
    }
    let mut out = PathBuf::new();
    for part in stack {
        out.push(part);
    }
    out.to_string_lossy().to_string()
}

fn exists_dir(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_dir(),
        Err(_) => false,
    }
}

fn join_skills_dir(root: &Path, skills_dir: &str) -> PathBuf {
    if skills_dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(skills_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_targets_prefers_project() {
        let agent = Agent {
            id: "codex".to_string(),
            display_name: "Codex".to_string(),
            global_config: ".codex".to_string(),
            project_config: ".codex".to_string(),
            skills_dir: "skills".to_string(),
        };
        let project = Target {
            agent: agent.clone(),
            scope: Scope::Project,
            root: PathBuf::from("/tmp/project"),
            skills_dir: PathBuf::from("/tmp/project/skills"),
        };
        let global = Target {
            agent: agent.clone(),
            scope: Scope::Global,
            root: PathBuf::from("/tmp/global"),
            skills_dir: PathBuf::from("/tmp/global/skills"),
        };
        let targets = vec![global, project];
        let defaults = default_targets(&targets);
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].scope, Scope::Project);
    }
}
