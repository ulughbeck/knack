use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "globalConfig")]
    pub global_config: String,
    #[serde(rename = "projectConfig")]
    pub project_config: String,
    #[serde(rename = "skillsDir")]
    pub skills_dir: String,
}

pub fn default_agents() -> Vec<Agent> {
    vec![
        Agent {
            id: "codex".to_string(),
            display_name: "Codex".to_string(),
            global_config: ".codex".to_string(),
            project_config: ".codex".to_string(),
            skills_dir: "skills".to_string(),
        },
        Agent {
            id: "claude".to_string(),
            display_name: "Claude".to_string(),
            global_config: ".claude".to_string(),
            project_config: ".claude".to_string(),
            skills_dir: "skills".to_string(),
        },
        Agent {
            id: "amp".to_string(),
            display_name: "Amp".to_string(),
            global_config: ".config/agents".to_string(),
            project_config: ".agents".to_string(),
            skills_dir: "skills".to_string(),
        },
        Agent {
            id: "opencode".to_string(),
            display_name: "OpenCode".to_string(),
            global_config: ".config/opencode".to_string(),
            project_config: ".opencode".to_string(),
            skills_dir: "skill".to_string(),
        },
        Agent {
            id: "goose".to_string(),
            display_name: "Goose".to_string(),
            global_config: ".config/goose".to_string(),
            project_config: ".goose".to_string(),
            skills_dir: "skills".to_string(),
        },
        Agent {
            id: "copilot".to_string(),
            display_name: "GitHub Copilot".to_string(),
            global_config: String::new(),
            project_config: ".github".to_string(),
            skills_dir: "skills".to_string(),
        },
        Agent {
            id: "letta".to_string(),
            display_name: "Letta".to_string(),
            global_config: String::new(),
            project_config: ".skills".to_string(),
            skills_dir: String::new(),
        },
        Agent {
            id: "cursor".to_string(),
            display_name: "Cursor".to_string(),
            global_config: ".cursor".to_string(),
            project_config: ".cursor".to_string(),
            skills_dir: "skills".to_string(),
        },
    ]
}

pub(crate) fn merge_agents(defaults: &[Agent], overrides: &[Agent]) -> Vec<Agent> {
    let mut by_id = std::collections::HashMap::new();
    let mut order = Vec::new();

    for agent in defaults {
        if agent.id.is_empty() {
            continue;
        }
        by_id.insert(agent.id.clone(), agent.clone());
        order.push(agent.id.clone());
    }

    for agent in overrides {
        if agent.id.is_empty() {
            continue;
        }
        if !by_id.contains_key(&agent.id) {
            order.push(agent.id.clone());
        }
        by_id.insert(agent.id.clone(), agent.clone());
    }

    let mut merged = Vec::with_capacity(by_id.len());
    for id in order {
        if let Some(agent) = by_id.get(&id) {
            merged.push(agent.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_agents_override() {
        let defaults = vec![
            Agent {
                id: "codex".to_string(),
                display_name: "Codex".to_string(),
                global_config: ".codex".to_string(),
                project_config: ".codex".to_string(),
                skills_dir: "skills".to_string(),
            },
            Agent {
                id: "claude".to_string(),
                display_name: "Claude".to_string(),
                global_config: ".claude".to_string(),
                project_config: ".claude".to_string(),
                skills_dir: "skills".to_string(),
            },
        ];
        let overrides = vec![
            Agent {
                id: "codex".to_string(),
                display_name: "CodexX".to_string(),
                global_config: ".codexx".to_string(),
                project_config: ".codexx".to_string(),
                skills_dir: "skills".to_string(),
            },
            Agent {
                id: "new".to_string(),
                display_name: "New".to_string(),
                global_config: ".new".to_string(),
                project_config: ".new".to_string(),
                skills_dir: "skills".to_string(),
            },
        ];

        let merged = merge_agents(&defaults, &overrides);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].display_name, "CodexX");
        assert_eq!(merged[2].id, "new");
    }
}
