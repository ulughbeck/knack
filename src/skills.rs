use crate::error::{KnackError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tempfile::{NamedTempFile, TempDir};
use url::Url;

#[derive(Debug)]
pub struct Source {
    pub root: PathBuf,
    pub subdir: Option<String>,
    pub url: String,
    _temp_dir: Option<TempDir>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDir {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "extraPaths")]
    pub extra_paths: Vec<String>,
}

pub fn fetch_source(input: &str) -> Result<Source> {
    if input.trim().is_empty() {
        return Err(KnackError::msg("empty source"));
    }

    let input_path = Path::new(input);
    if input_path.exists() {
        let meta = fs::metadata(input_path)?;
        if meta.is_dir() {
            return Ok(Source {
                root: input_path.to_path_buf(),
                subdir: None,
                url: input.to_string(),
                _temp_dir: None,
            });
        }
        if meta.is_file() {
            if input_path
                .file_name()
                .map(|n| n == std::ffi::OsStr::new("SKILL.md"))
                .unwrap_or(false)
            {
                let root = input_path
                    .parent()
                    .ok_or_else(|| KnackError::msg("invalid skill path"))?
                    .to_path_buf();
                return Ok(Source {
                    root,
                    subdir: None,
                    url: input.to_string(),
                    _temp_dir: None,
                });
            }
            return Err(KnackError::msg(format!("unsupported file path: {}", input)));
        }
    }

    if let Some((owner, repo, subdir)) = parse_github_shorthand(input) {
        let (root, temp_dir) = download_github_repo(&owner, &repo, None)?;
        return Ok(Source {
            root,
            subdir,
            url: input.to_string(),
            _temp_dir: Some(temp_dir),
        });
    }

    let parsed =
        Url::parse(input).map_err(|_| KnackError::msg(format!("unsupported source: {}", input)))?;

    if parsed.scheme().is_empty() {
        return Err(KnackError::msg(format!("unsupported source: {}", input)));
    }

    if parsed.host_str() == Some("github.com") {
        let (owner, repo, reference, subdir) = parse_github_url(&parsed)
            .ok_or_else(|| KnackError::msg(format!("unsupported GitHub URL: {}", input)))?;
        let (root, temp_dir) = download_github_repo(&owner, &repo, reference.as_deref())?;
        return Ok(Source {
            root,
            subdir,
            url: input.to_string(),
            _temp_dir: Some(temp_dir),
        });
    }

    if parsed.path().ends_with(".zip") {
        let (root, temp_dir) = download_zip(input)?;
        return Ok(Source {
            root,
            subdir: None,
            url: input.to_string(),
            _temp_dir: Some(temp_dir),
        });
    }

    Err(KnackError::msg(format!(
        "unsupported source URL: {}",
        input
    )))
}

pub fn find_skill_dirs(source: &Source, skill_name: Option<&str>) -> Result<Vec<SkillDir>> {
    let base = source_base_path(source)?;

    if let Some(name) = skill_name {
        if is_skill_dir(&base)? {
            let matches_dir_name = base
                .file_name()
                .map(|n| n == std::ffi::OsStr::new(name))
                .unwrap_or(false);
            let matches_preferred = preferred_root_skill_name(source).as_deref() == Some(name);
            if matches_dir_name || matches_preferred {
                return Ok(vec![SkillDir {
                    name: name.to_string(),
                    path: base.clone(),
                }]);
            }
        }
        let candidate = base.join(name);
        if is_skill_dir(&candidate)? {
            return Ok(vec![SkillDir {
                name: name.to_string(),
                path: candidate,
            }]);
        }
        return Err(KnackError::msg(format!("skill \"{}\" not found", name)));
    }

    let mut list = Vec::new();
    if is_skill_dir(&base)? {
        let name = preferred_root_skill_name(source).unwrap_or_else(|| {
            base.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "skill".to_string())
        });
        list.push(SkillDir {
            name,
            path: base.clone(),
        });
    }

    let entries = fs::read_dir(&base).map_err(|_| KnackError::msg("no skill directories found"))?;
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let candidate = entry.path();
        if is_skill_dir(&candidate)? {
            let name = entry.file_name().to_string_lossy().to_string();
            list.push(SkillDir {
                name,
                path: candidate,
            });
        }
    }
    if list.is_empty() {
        return Err(KnackError::msg("no skill directories found"));
    }
    Ok(list)
}

pub fn load_manifest(skill_path: &Path) -> Result<Manifest> {
    let manifest_path = skill_path.join("skill.json");
    let data = match fs::read(&manifest_path) {
        Ok(data) => data,
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                return Ok(Manifest::default());
            }
            return Err(err.into());
        }
    };
    let manifest: Manifest = serde_json::from_slice(&data)?;
    Ok(manifest)
}

pub fn copy_extras(manifest: &Manifest, source_root: &Path, dest_root: &Path) -> Result<()> {
    for rel_path in &manifest.extra_paths {
        let clean = sanitize_rel_path(rel_path)
            .ok_or_else(|| KnackError::msg(format!("invalid extra path: {}", rel_path)))?;
        let src = source_root.join(&clean);
        let dest = dest_root.join(&clean);
        copy_path(&src, &dest)?;
    }
    Ok(())
}

pub fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if !meta.is_dir() {
        return Err(KnackError::msg(format!(
            "source is not a directory: {}",
            src.display()
        )));
    }
    copy_dir_inner(src, dest, &meta)
}

pub fn copy_path(src: &Path, dest: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        return copy_dir(src, dest);
    }
    copy_file(src, dest, &meta)
}

fn copy_dir_inner(src: &Path, dest: &Path, _meta: &fs::Metadata) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let meta = entry.metadata()?;
        if file_type.is_dir() {
            copy_dir_inner(&path, &target, &meta)?;
        } else {
            copy_file(&path, &target, &meta)?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dest, fs::Permissions::from_mode(_meta.permissions().mode()))?;
    }
    Ok(())
}

fn copy_file(src: &Path, dest: &Path, _meta: &fs::Metadata) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut input = fs::File::open(src)?;
    let mut output = fs::File::create(dest)?;
    io::copy(&mut input, &mut output)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dest, fs::Permissions::from_mode(_meta.permissions().mode()))?;
    }
    Ok(())
}

fn is_skill_dir(dir: &Path) -> Result<bool> {
    let meta = match fs::metadata(dir) {
        Ok(meta) => meta,
        Err(_) => return Ok(false),
    };
    if !meta.is_dir() {
        return Ok(false);
    }
    Ok(dir.join("SKILL.md").is_file())
}

fn source_base_path(source: &Source) -> Result<PathBuf> {
    if let Some(subdir) = &source.subdir {
        let clean = sanitize_rel_path_allow_empty(subdir)
            .ok_or_else(|| KnackError::msg("invalid source subdir"))?;
        if clean.as_os_str().is_empty() {
            return Ok(source.root.clone());
        }
        return Ok(source.root.join(clean));
    }
    Ok(source.root.clone())
}

fn parse_github_shorthand(input: &str) -> Option<(String, String, Option<String>)> {
    if input.contains("://") {
        return None;
    }
    if input.starts_with('/') || input.starts_with("./") || input.starts_with("../") {
        return None;
    }
    let parts: Vec<&str> = input.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return None;
    }
    let owner = parts[0].to_string();
    let repo = parts[1].trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let mut subdir = if parts.len() > 2 {
        let joined = parts[2..].join("/");
        clean_subdir(&joined)?
    } else {
        None
    };
    if let Some(dir) = subdir.as_deref() {
        if dir == "SKILL.md" || dir.ends_with("/SKILL.md") {
            let parent = parent_subdir(dir);
            subdir = if parent.is_empty() {
                None
            } else {
                Some(parent)
            };
        }
    }
    Some((owner, repo, subdir))
}

fn parse_github_url(url: &Url) -> Option<(String, String, Option<String>, Option<String>)> {
    let parts: Vec<&str> = url.path().trim_matches('/').split('/').collect();
    if parts.len() < 2 {
        return None;
    }
    let owner = parts[0].to_string();
    let repo = parts[1].trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    let mut reference: Option<String> = None;
    let mut subdir: Option<String> = None;

    if parts.len() >= 4 && parts[2] == "tree" {
        reference = Some(parts[3].to_string());
        if parts.len() > 4 {
            let joined = parts[4..].join("/");
            subdir = clean_subdir(&joined)?;
            if let Some(dir) = subdir.as_deref() {
                if dir == "SKILL.md" || dir.ends_with("/SKILL.md") {
                    let parent = parent_subdir(dir);
                    subdir = if parent.is_empty() {
                        None
                    } else {
                        Some(parent)
                    };
                }
            }
        }
    }

    if parts.len() >= 4 && parts[2] == "blob" {
        if parts.last().copied()? != "SKILL.md" {
            return None;
        }
        reference = Some(parts[3].to_string());
        if parts.len() > 4 {
            let joined = parts[4..parts.len() - 1].join("/");
            if !joined.is_empty() {
                subdir = clean_subdir(&joined)?;
            }
        }
    }

    Some((owner, repo, reference, subdir))
}

fn preferred_root_skill_name(source: &Source) -> Option<String> {
    if source.subdir.is_some() {
        return None;
    }
    if let Some((_, repo, _)) = parse_github_shorthand(&source.url) {
        return Some(repo);
    }
    if let Ok(parsed) = Url::parse(&source.url) {
        if parsed.host_str() == Some("github.com") {
            if let Some((_, repo, _, subdir)) = parse_github_url(&parsed) {
                if subdir.is_none() {
                    return Some(repo);
                }
            }
        }
    }
    None
}

fn download_github_repo(
    owner: &str,
    repo: &str,
    reference: Option<&str>,
) -> Result<(PathBuf, TempDir)> {
    let zip_url = match reference {
        Some(reference) => format!(
            "https://api.github.com/repos/{}/{}/zipball/{}",
            owner, repo, reference
        ),
        None => format!("https://api.github.com/repos/{}/{}/zipball", owner, repo),
    };
    download_zip(&zip_url)
}

fn download_zip(zip_url: &str) -> Result<(PathBuf, TempDir)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    let mut resp = client.get(zip_url).header("User-Agent", "knack").send()?;
    if !resp.status().is_success() {
        return Err(KnackError::msg(format!(
            "failed to download ({}): {}",
            zip_url,
            resp.status()
        )));
    }

    let mut zip_file = NamedTempFile::new()?;
    io::copy(&mut resp, &mut zip_file)?;
    zip_file.flush()?;

    let file = zip_file.reopen()?;
    let mut archive = zip::ZipArchive::new(file)?;
    let temp_dir = tempfile::tempdir()?;
    extract_zip(&mut archive, temp_dir.path())?;
    let root = resolve_extracted_root(temp_dir.path());
    Ok((root, temp_dir))
}

fn extract_zip<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, dest: &Path) -> Result<()> {
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let clean = sanitize_zip_path(&name)?;
        if clean.as_os_str().is_empty() {
            return Err(KnackError::msg(format!("invalid zip entry: {}", name)));
        }
        let target = dest.join(&clean);
        if file.is_dir() || name.ends_with('/') {
            fs::create_dir_all(&target)?;
            #[cfg(unix)]
            if let Some(mode) = file.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&target, fs::Permissions::from_mode(mode))?;
            }
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = fs::File::create(&target)?;
        io::copy(&mut file, &mut outfile)?;
        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(())
}

fn sanitize_zip_path(name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(KnackError::msg(format!("invalid zip entry: {}", name)));
            }
        }
    }
    Ok(clean)
}

fn resolve_extracted_root(parent: &Path) -> PathBuf {
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries
            .collect::<std::result::Result<Vec<_>, std::io::Error>>()
            .ok(),
        Err(_) => None,
    };
    if let Some(entries) = entries {
        if entries.len() == 1 {
            if let Ok(meta) = entries[0].metadata() {
                if meta.is_dir() {
                    return entries[0].path();
                }
            }
        }
    }
    parent.to_path_buf()
}

fn sanitize_rel_path(path: &str) -> Option<PathBuf> {
    sanitize_rel_path_allow_empty(path).and_then(|clean| {
        if clean.as_os_str().is_empty() {
            None
        } else {
            Some(clean)
        }
    })
}

fn sanitize_rel_path_allow_empty(path: &str) -> Option<PathBuf> {
    if Path::new(path).is_absolute() {
        return None;
    }
    clean_relative_path(path)
}

fn clean_relative_path(path: &str) -> Option<PathBuf> {
    let mut stack: Vec<PathBuf> = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => stack.push(PathBuf::from(part)),
            Component::CurDir => {}
            Component::ParentDir => {
                if stack.pop().is_none() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let mut clean = PathBuf::new();
    for part in stack {
        clean.push(part);
    }
    Some(clean)
}

fn clean_subdir(subdir: &str) -> Option<Option<String>> {
    let clean = sanitize_rel_path_allow_empty(subdir)?;
    if clean.as_os_str().is_empty() {
        return Some(None);
    }
    Some(Some(clean.to_string_lossy().to_string()))
}

fn parent_subdir(subdir: &str) -> String {
    let path = Path::new(subdir);
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_string_lossy().to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_skill_dirs_root() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("SKILL.md"), "test").unwrap();
        let source = Source {
            root: dir.path().to_path_buf(),
            subdir: None,
            url: dir.path().to_string_lossy().to_string(),
            _temp_dir: None,
        };
        let skills = find_skill_dirs(&source, None).unwrap();
        assert_eq!(skills.len(), 1);
    }

    #[test]
    fn find_skill_dirs_child_folder() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("alpha");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join("SKILL.md"), "test").unwrap();
        let source = Source {
            root: dir.path().to_path_buf(),
            subdir: None,
            url: dir.path().to_string_lossy().to_string(),
            _temp_dir: None,
        };
        let skills = find_skill_dirs(&source, None).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "alpha");
    }

    #[test]
    fn find_skill_dirs_with_name() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("beta");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join("SKILL.md"), "test").unwrap();
        let source = Source {
            root: dir.path().to_path_buf(),
            subdir: None,
            url: dir.path().to_string_lossy().to_string(),
            _temp_dir: None,
        };
        let skills = find_skill_dirs(&source, Some("beta")).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "beta");
    }

    #[test]
    fn parse_github_shorthand_cases() {
        let cases = vec![
            ("anthropics/skills", "anthropics", "skills", "", true),
            ("anthropics/skills/pdf", "anthropics", "skills", "pdf", true),
            (
                "anthropics/skills/pdf/SKILL.md",
                "anthropics",
                "skills",
                "pdf",
                true,
            ),
            (
                "github.com/anthropics/skills/pdf",
                "github.com",
                "anthropics",
                "skills/pdf",
                true,
            ),
            ("./skills", "", "", "", false),
        ];
        for (input, owner, repo, subdir, ok) in cases {
            let parsed = parse_github_shorthand(input);
            assert_eq!(parsed.is_some(), ok, "input {}", input);
            if let Some((got_owner, got_repo, got_subdir)) = parsed {
                assert_eq!(got_owner, owner);
                assert_eq!(got_repo, repo);
                let normalized = got_subdir.unwrap_or_default().replace('\\', "/");
                assert_eq!(normalized, subdir);
            }
        }
    }
}
