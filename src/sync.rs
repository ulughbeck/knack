use crate::config::Settings;
use crate::detector::{Scope, Target};
use crate::error::{KnackError, Result};
use crate::skills;
use std::fs;
use std::path::{Path, PathBuf};

pub fn resolve_skills_root(settings: &Settings, config_path: &Path) -> Result<PathBuf> {
    if let Some(root) = settings.skills_root.as_deref() {
        return resolve_user_path(root);
    }
    if let Some(parent) = config_path.parent() {
        return Ok(parent.join("skills"));
    }
    Err(KnackError::msg("unable to resolve skills root"))
}

pub fn install_skill_from_cache(
    cache_root: &Path,
    skill_name: &str,
    targets: &[Target],
    force: bool,
) -> Result<()> {
    let cache_path = cache_root.join(skill_name);
    if !cache_path.exists() {
        return Err(KnackError::msg(format!(
            "cached skill \"{}\" not found",
            skill_name
        )));
    }

    let manifest = skills::load_manifest(&cache_path)?;
    for target in targets {
        if target.scope != Scope::Global {
            continue;
        }
        fs::create_dir_all(&target.skills_dir)?;
        let dest = target.skills_dir.join(skill_name);
        let dest_meta = fs::symlink_metadata(&dest).ok();
        let should_copy = match dest_meta {
            None => true,
            Some(meta) => {
                if meta.is_dir() && !force {
                    false
                } else {
                    remove_path(&dest)?;
                    true
                }
            }
        };
        if should_copy {
            skills::copy_dir(&cache_path, &dest)?;
        }
        if !manifest.extra_paths.is_empty() && (should_copy || force) {
            skills::copy_extras(&manifest, &cache_path, &target.root)?;
        }
    }

    Ok(())
}

fn resolve_user_path(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let home =
        dirs::home_dir().ok_or_else(|| KnackError::msg("unable to resolve home directory"))?;
    if raw == "~" {
        return Ok(home);
    }
    if let Some(stripped) = raw.strip_prefix("~/") {
        return Ok(home.join(stripped));
    }
    Ok(home.join(path))
}

fn remove_path(path: &Path) -> Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(err.into());
        }
    };
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
