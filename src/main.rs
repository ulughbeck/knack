use clap::{Parser, Subcommand};
use knack::cli;
use knack::config;
use knack::detector::{self, Scope, Target};
use knack::registry::Agent;
use knack::skills;
use knack::sync;
use knack::{KnackError, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "knack", version, about = "Manage agent skills for your agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        source: String,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        project: bool,
        #[arg(long)]
        force: bool,
    },
    #[command(name = "rm")]
    Rm {
        skill: String,
        #[arg(long)]
        project: bool,
    },
    Doctor,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Add {
            source,
            skill,
            agent,
            project,
            force,
        } => run_add(&source, skill.as_deref(), agent.as_deref(), project, force),
        Commands::Rm { skill, project } => run_rm(&skill, project),
        Commands::Doctor => run_doctor(),
    };
    std::process::exit(code);
}

fn run_add(
    source_input: &str,
    skill_name: Option<&str>,
    agent_id: Option<&str>,
    force_project: bool,
    force_flag: bool,
) -> i32 {
    let (mut config, targets, _global_targets) = match load_config_and_targets() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };

    let scope = if force_project {
        Some(Scope::Project)
    } else {
        None
    };
    let targets = match resolve_targets_from(&config.agents, &targets, scope, agent_id) {
        Ok(targets) => targets,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };

    let mut progress = cli::Progress::new();
    let _ = progress.start("Fetching source...");
    let source = match skills::fetch_source(source_input) {
        Ok(source) => source,
        Err(err) => {
            let _ = progress.finish("");
            eprintln!("{}", err);
            return 1;
        }
    };

    let _ = progress.update("Searching for skills...");
    let skill_dirs = match skills::find_skill_dirs(&source, skill_name) {
        Ok(list) => list,
        Err(err) => {
            let _ = progress.finish("");
            eprintln!("{}", err);
            return 1;
        }
    };
    if skill_dirs.len() == 1 {
        let _ = progress.update(&format!("Found skill: {}", skill_dirs[0].name));
    } else {
        let _ = progress.update(&format!("Found {} skills.", skill_dirs.len()));
    }
    brief_pause();
    let _ = progress.finish("");

    let selected_skills = match select_skill_dirs(&skill_dirs) {
        Ok(skills) => skills,
        Err(err) => {
            let _ = progress.finish("");
            eprintln!("{}", err);
            return 1;
        }
    };

    let mut install_queue = Vec::new();
    for skill in selected_skills {
        let existing_targets = match find_existing_skill_targets(&targets, &skill.name) {
            Ok(list) => list,
            Err(err) => {
                eprintln!("{}", err);
                return 1;
            }
        };

        let mut force = force_flag;
        if !existing_targets.is_empty() && !force {
            if !cli::is_interactive() {
                eprintln!("skill already exists; use --force to overwrite");
                return 1;
            }
            let choices = vec![
                "Overwrite existing skills".to_string(),
                "Do nothing".to_string(),
            ];
            let prompt = format!(
                "Skill \"{}\" already installed for: {}",
                skill.name,
                format_target_list(&existing_targets)
            );
            let choice = match cli::select_one(&prompt, &choices, Some(1)) {
                Ok(choice) => choice,
                Err(err) => {
                    eprintln!("{}", err);
                    return 1;
                }
            };
            if choice == 1 {
                continue;
            }
            force = true;
        }
        install_queue.push((skill, force));
    }

    if install_queue.is_empty() {
        return 0;
    }

    let mut project_targets = Vec::new();
    let mut global_selected = Vec::new();
    for target in targets {
        if target.scope == Scope::Project {
            project_targets.push(target);
        } else {
            global_selected.push(target);
        }
    }

    for (skill, force) in &install_queue {
        if !project_targets.is_empty() {
            let _ = progress.start(&format!(
                "Installing {} into project targets...",
                skill.name
            ));
            brief_pause();
            let _ = progress.finish("");
        }
        for target in &project_targets {
            let _ = progress.start(&format!(
                "Installing {} into {} ({})...",
                skill.name,
                target.agent.display_name,
                target.scope.as_str()
            ));
            if let Err(err) = install_project_skill(target, skill, &source, *force) {
                let _ = progress.finish("");
                eprintln!(
                    "Add failed for {} ({}): {}",
                    target.agent.display_name,
                    target.scope.as_str(),
                    err
                );
                return 1;
            }
            let _ = progress.finish("");
            println!(
                "Added {} to {} ({})",
                skill.name,
                target.agent.display_name,
                target.scope.as_str()
            );
        }
    }

    if !global_selected.is_empty() {
        let skills_root = match sync::resolve_skills_root(&config.settings, &config.path) {
            Ok(root) => root,
            Err(err) => {
                eprintln!("{}", err);
                return 1;
            }
        };
        if let Err(err) = fs::create_dir_all(&skills_root) {
            eprintln!("{}", err);
            return 1;
        }

        let mut config_changed = false;
        for (skill, force) in &install_queue {
            if let Err(err) = update_global_config(
                &mut config,
                &skill.name,
                &source.url,
                &global_selected,
                *force,
            ) {
                eprintln!("{}", err);
                return 1;
            }
            config_changed = true;
            let _ = progress.start(&format!("Caching skill {}...", skill.name));
            brief_pause();
            if let Err(err) = cache_selected_skill(&config, skill, &source, *force) {
                let _ = progress.finish("");
                eprintln!("{}", err);
                return 1;
            }
            let _ = progress.finish("");

            for target in &global_selected {
                let _ = progress.start(&format!(
                    "Installing {} into {} ({})...",
                    skill.name,
                    target.agent.display_name,
                    target.scope.as_str()
                ));
                if let Err(err) = sync::install_skill_from_cache(
                    &skills_root,
                    &skill.name,
                    std::slice::from_ref(target),
                    *force,
                ) {
                    let _ = progress.finish("");
                    eprintln!("{}", err);
                    return 1;
                }
                let _ = progress.finish("");
                println!(
                    "Added {} to {} ({})",
                    skill.name,
                    target.agent.display_name,
                    target.scope.as_str()
                );
            }
        }

        if config_changed {
            if let Err(err) = config::save(&config) {
                eprintln!("{}", err);
                return 1;
            }
        }
    }

    0
}

fn brief_pause() {
    thread::sleep(Duration::from_millis(120));
}

fn run_rm(skill_name: &str, force_project: bool) -> i32 {
    let (mut config, targets, _global_targets) = match load_config_and_targets() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };

    let scope = if force_project {
        Scope::Project
    } else {
        Scope::Global
    };

    let project_targets = detector::filter_scope(&targets, Scope::Project);
    if scope == Scope::Project && project_targets.is_empty() {
        println!("No project configs found. Run inside a repo with agent config(s).");
        return 0;
    }

    let mut candidates = if scope == Scope::Project {
        project_targets.clone()
    } else {
        targets.clone()
    };

    candidates.retain(|target| match target.scope {
        Scope::Global => {
            let allowed = config
                .skills
                .iter()
                .find(|spec| spec.name == skill_name)
                .map(|spec| spec.agents.clone())
                .unwrap_or_default();
            allowed.contains(&target.agent.id)
        }
        Scope::Project => {
            let path = target.skills_dir.join(skill_name);
            fs::metadata(&path).is_ok()
        }
    });

    if candidates.is_empty() {
        if scope == Scope::Project {
            println!("Skill {} not found in project skills.", skill_name);
        } else {
            println!("Skill {} not found in selected targets.", skill_name);
        }
        return 0;
    }

    let targets = if candidates.len() == 1 {
        candidates
    } else if !cli::is_interactive() {
        order_targets(candidates)
    } else {
        let ui = cli::SelectorUi::multi()
            .with_cancel("skill remove cancelled")
            .with_empty("no targets selected")
            .with_confirm_label("Remove")
            .with_footer("↑/↓ move · space:toggle · a:all · n:none · Enter:remove · Esc:cancel");
        match prompt_targets(
            &candidates,
            &format!("Remove skill {} from agents:", skill_name),
            ui,
        ) {
            Ok(list) => list,
            Err(err) => {
                eprintln!("{}", err);
                return 1;
            }
        }
    };

    let mut project_targets = Vec::new();
    let mut global_selected = Vec::new();
    for target in targets {
        if target.scope == Scope::Project {
            project_targets.push(target);
        } else {
            global_selected.push(target);
        }
    }

    for target in &project_targets {
        let path = target.skills_dir.join(skill_name);
        match fs::metadata(&path) {
            Ok(_) => {
                let remove_result = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
                if let Err(err) = remove_result {
                    eprintln!(
                        "Failed to remove from {} ({}): {}",
                        target.agent.display_name,
                        target.scope.as_str(),
                        err
                    );
                    return 1;
                }
                println!(
                    "Removed {} from {} ({})",
                    skill_name,
                    target.agent.display_name,
                    target.scope.as_str()
                );
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    println!(
                        "Skill {} not found in {} ({})",
                        skill_name,
                        target.agent.display_name,
                        target.scope.as_str()
                    );
                    continue;
                }
                eprintln!(
                    "Failed to check skill in {} ({}): {}",
                    target.agent.display_name,
                    target.scope.as_str(),
                    err
                );
                return 1;
            }
        }
    }

    if !global_selected.is_empty() {
        for target in &global_selected {
            let path = target.skills_dir.join(skill_name);
            match fs::metadata(&path) {
                Ok(_) => {
                    let remove_result = if path.is_dir() {
                        fs::remove_dir_all(&path)
                    } else {
                        fs::remove_file(&path)
                    };
                    if let Err(err) = remove_result {
                        eprintln!(
                            "Failed to remove from {} ({}): {}",
                            target.agent.display_name,
                            target.scope.as_str(),
                            err
                        );
                        return 1;
                    }
                    println!(
                        "Removed {} from {} ({})",
                        skill_name,
                        target.agent.display_name,
                        target.scope.as_str()
                    );
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        continue;
                    }
                    eprintln!(
                        "Failed to check skill in {} ({}): {}",
                        target.agent.display_name,
                        target.scope.as_str(),
                        err
                    );
                    return 1;
                }
            }
        }
        if let Err(err) = remove_global_skill(&mut config, skill_name, &global_selected) {
            eprintln!("{}", err);
            return 1;
        }
        if let Err(err) = config::save(&config) {
            eprintln!("{}", err);
            return 1;
        }
        let still_present = config.skills.iter().any(|spec| spec.name == skill_name);
        if !still_present {
            let skills_root = match sync::resolve_skills_root(&config.settings, &config.path) {
                Ok(root) => root,
                Err(err) => {
                    eprintln!("{}", err);
                    return 1;
                }
            };
            let cache_path = skills_root.join(skill_name);
            if cache_path.exists() {
                if cache_path.is_dir() {
                    if let Err(err) = fs::remove_dir_all(&cache_path) {
                        eprintln!("{}", err);
                        return 1;
                    }
                } else if let Err(err) = fs::remove_file(&cache_path) {
                    eprintln!("{}", err);
                    return 1;
                }
            }
        }
    }

    0
}

fn run_doctor() -> i32 {
    let (_config, targets, _global_targets) = match load_config_and_targets() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };

    let global_targets = detector::filter_scope(&targets, Scope::Global);
    let project_targets = detector::filter_scope(&targets, Scope::Project);

    print_doctor_section("Agents found:", &global_targets);
    if !project_targets.is_empty() {
        println!("");
        print_doctor_section("Project settings:", &project_targets);
    }

    0
}

fn load_config_and_targets() -> Result<(config::ConfigState, Vec<Target>, Vec<Target>)> {
    let config = config::load()?;
    let cwd = std::env::current_dir()?;
    let (targets, _) = detector::detect_targets(&config.agents, &cwd);
    let global_targets = detector::filter_scope(&targets, Scope::Global);
    Ok((config, targets, global_targets))
}

fn print_doctor_section(title: &str, targets: &[Target]) {
    println!("{}", title);
    if targets.is_empty() {
        println!("- None detected.");
        return;
    }
    for target in targets {
        let (count, warning) = count_skills(&target.skills_dir);
        if let Some(warning) = warning {
            println!(
                "- {} (skills={}, warning={})",
                target.agent.display_name, count, warning
            );
            continue;
        }
        println!("- {} (skills={})", target.agent.display_name, count);
    }
}

fn resolve_targets_from(
    agents: &[Agent],
    targets: &[Target],
    force_scope: Option<Scope>,
    agent_id: Option<&str>,
) -> Result<Vec<Target>> {
    if let Some(id) = agent_id {
        if !agent_exists(agents, id) {
            return Err(KnackError::msg(format!("unknown agent id: {}", id)));
        }
    }

    let mut targets = targets.to_vec();
    if let Some(id) = agent_id {
        targets = filter_targets_by_agent(&targets, id);
    }
    if let Some(scope) = force_scope.as_ref() {
        targets = detector::filter_scope(&targets, scope.clone());
    }

    if targets.is_empty() {
        if force_scope.as_ref() == Some(&Scope::Project) {
            return Err(KnackError::msg(
                "No project configs found. Run inside a repo with agent config(s).",
            ));
        }
        return Err(KnackError::msg("no agent targets found"));
    }
    if targets.len() == 1 {
        return Ok(targets);
    }
    if !cli::is_interactive() {
        return Ok(order_targets(targets));
    }

    let ui = cli::SelectorUi::multi()
        .with_cancel("skill install cancelled")
        .with_empty("no targets selected");
    prompt_targets(&targets, "Select agents:", ui)
}

fn order_targets(mut targets: Vec<Target>) -> Vec<Target> {
    targets.sort_by_key(|target| match target.scope {
        Scope::Global => 0,
        Scope::Project => 1,
    });
    targets
}

fn update_global_config(
    config: &mut config::ConfigState,
    skill_name: &str,
    source_url: &str,
    targets: &[Target],
    force: bool,
) -> Result<()> {
    if targets.is_empty() {
        return Ok(());
    }
    let mut agent_ids = Vec::new();
    for target in targets {
        agent_ids.push(target.agent.id.clone());
        if !config
            .agents
            .iter()
            .any(|agent| agent.id == target.agent.id)
        {
            config.agents.push(target.agent.clone());
        }
    }

    if let Some(spec) = config
        .skills
        .iter_mut()
        .find(|spec| spec.name == skill_name)
    {
        if let Some(existing) = spec.source.as_deref() {
            if existing != source_url && !force {
                return Err(KnackError::msg(format!(
                    "skill \"{}\" already exists with a different source; use --force to overwrite",
                    skill_name
                )));
            }
        }
        if spec.source.is_none() || force {
            spec.source = Some(source_url.to_string());
        }
        for agent_id in agent_ids {
            if !spec.agents.contains(&agent_id) {
                spec.agents.push(agent_id);
            }
        }
    } else {
        config.skills.push(config::SkillSpec {
            name: skill_name.to_string(),
            source: Some(source_url.to_string()),
            agents: agent_ids,
        });
    }

    config.exists = true;
    Ok(())
}

fn cache_selected_skill(
    config: &config::ConfigState,
    skill: &skills::SkillDir,
    source: &skills::Source,
    force: bool,
) -> Result<()> {
    let skills_root = sync::resolve_skills_root(&config.settings, &config.path)?;
    fs::create_dir_all(&skills_root)?;
    let cache_path = skills_root.join(&skill.name);
    if cache_path.exists() {
        if !force {
            return Ok(());
        }
        if cache_path.is_dir() {
            fs::remove_dir_all(&cache_path)?;
        } else {
            fs::remove_file(&cache_path)?;
        }
    }
    skills::copy_dir(&skill.path, &cache_path)?;
    let manifest = skills::load_manifest(&skill.path)?;
    if !manifest.extra_paths.is_empty() {
        skills::copy_extras(&manifest, &source.root, &cache_path)?;
    }
    Ok(())
}

fn remove_global_skill(
    config: &mut config::ConfigState,
    skill_name: &str,
    targets: &[Target],
) -> Result<()> {
    let Some(idx) = config
        .skills
        .iter()
        .position(|spec| spec.name == skill_name)
    else {
        return Ok(());
    };
    let remove_ids: HashSet<String> = targets.iter().map(|t| t.agent.id.clone()).collect();
    let spec = &mut config.skills[idx];
    spec.agents.retain(|id| !remove_ids.contains(id));
    if spec.agents.is_empty() {
        config.skills.remove(idx);
    }
    config.exists = true;
    Ok(())
}

fn agent_exists(agents: &[Agent], id: &str) -> bool {
    agents.iter().any(|agent| agent.id == id)
}

fn filter_targets_by_agent(targets: &[Target], id: &str) -> Vec<Target> {
    targets
        .iter()
        .cloned()
        .filter(|target| target.agent.id == id)
        .collect()
}

fn prompt_targets(
    targets: &[Target],
    prompt: &str,
    ui: cli::SelectorUi<'_>,
) -> Result<Vec<Target>> {
    let mut ordered: Vec<Target> = targets.to_vec();
    ordered.sort_by_key(|target| match target.scope {
        Scope::Global => 0,
        Scope::Project => 1,
    });

    let default_indexes: Vec<usize> = (0..ordered.len()).collect();
    let mut options = Vec::new();
    for target in &ordered {
        let scope = target.scope.as_str();
        options.push(format!(
            "{} ({}) - {}",
            target.agent.display_name,
            scope,
            target.root.display()
        ));
    }

    let indexes = cli::select_many_with_ui(prompt, &options, &default_indexes, ui)?;
    if indexes.is_empty() {
        return Err(KnackError::msg("no targets selected"));
    }
    let mut selection = Vec::with_capacity(indexes.len());
    for idx in indexes {
        if let Some(target) = ordered.get(idx) {
            selection.push(target.clone());
        }
    }
    Ok(selection)
}

fn select_skill_dirs(skills_list: &[skills::SkillDir]) -> Result<Vec<skills::SkillDir>> {
    if skills_list.len() == 1 {
        return Ok(vec![skills_list[0].clone()]);
    }
    if !cli::is_interactive() {
        return Err(KnackError::msg("multiple skills found; rerun with --skill"));
    }

    let options: Vec<String> = skills_list.iter().map(|skill| skill.name.clone()).collect();
    let default_indexes: Vec<usize> = (0..skills_list.len()).collect();
    let ui = cli::SelectorUi::multi().with_empty("no skills selected");
    let indexes = cli::select_many_with_ui("Select skills:", &options, &default_indexes, ui)?;
    if indexes.is_empty() {
        return Err(KnackError::msg("no skills selected"));
    }
    let mut selected = Vec::with_capacity(indexes.len());
    for idx in indexes {
        if let Some(skill) = skills_list.get(idx) {
            selected.push(skill.clone());
        }
    }
    Ok(selected)
}

fn find_existing_skill_targets(targets: &[Target], skill_name: &str) -> Result<Vec<Target>> {
    let mut existing = Vec::new();
    for target in targets {
        let path = target.skills_dir.join(skill_name);
        match fs::metadata(&path) {
            Ok(_) => existing.push(target.clone()),
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    continue;
                }
                return Err(err.into());
            }
        }
    }
    Ok(existing)
}

fn format_target_list(targets: &[Target]) -> String {
    let parts: Vec<String> = targets
        .iter()
        .map(|target| format!("{} ({})", target.agent.display_name, target.scope.as_str()))
        .collect();
    parts.join(", ")
}

fn install_project_skill(
    target: &Target,
    skill: &skills::SkillDir,
    source: &skills::Source,
    force: bool,
) -> Result<()> {
    fs::create_dir_all(&target.skills_dir)?;
    let dest = target.skills_dir.join(&skill.name);
    if dest.exists() {
        if !force {
            return Err(KnackError::msg(
                "skill already exists; use --force to overwrite",
            ));
        }
        if dest.is_dir() {
            fs::remove_dir_all(&dest)?;
        } else {
            fs::remove_file(&dest)?;
        }
    }
    skills::copy_dir(&skill.path, &dest)?;
    let manifest = skills::load_manifest(&skill.path)?;
    if !manifest.extra_paths.is_empty() {
        skills::copy_extras(&manifest, &source.root, &target.root)?;
    }
    Ok(())
}

fn count_skills(dir: &Path) -> (usize, Option<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                return (0, None);
            }
            return (0, Some(err.to_string()));
        }
    };
    let mut count = 0;
    for entry in entries {
        if let Ok(entry) = entry {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".system" {
                continue;
            }
            if entry.metadata().map(|meta| meta.is_dir()).unwrap_or(false) {
                count += 1;
            }
        }
    }
    (count, None)
}
