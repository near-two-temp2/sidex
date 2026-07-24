use super::validation::validate_path;
use std::path::{Path, PathBuf};

#[cfg(windows)]
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn basename(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| "folder".to_string(), |n| n.to_string_lossy().into_owned())
}

fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        return "folder-group".to_string();
    }
    #[cfg(windows)]
    if RESERVED_DEVICE_NAMES
        .iter()
        .any(|r| trimmed.eq_ignore_ascii_case(r))
    {
        return format!("{trimmed}-group");
    }
    trimmed.to_string()
}

fn default_group_name(folders: &[String]) -> String {
    let joined = folders
        .iter()
        .take(2)
        .map(|f| basename(Path::new(f)))
        .collect::<Vec<_>>()
        .join("-");
    if folders.len() > 2 {
        format!("{joined}-and-{}-more", folders.len() - 2)
    } else {
        joined
    }
}

/// First non-colliding path in `dir` for `name`, appending -2, -3... as needed.
/// Uses `symlink_metadata` so broken links still count as occupied.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let mut candidate = dir.join(name);
    let mut counter = 2;
    while candidate.symlink_metadata().is_ok() {
        candidate = dir.join(format!("{name}-{counter}"));
        counter += 1;
    }
    candidate
}

#[cfg(windows)]
fn simplified(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

#[cfg(not(windows))]
fn simplified(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(unix)]
fn make_link(target: &Path, link: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|e| {
        format!(
            "failed to link {} -> {}: {e}",
            link.display(),
            target.display()
        )
    })
}

#[cfg(windows)]
fn make_link(target: &Path, link: &Path) -> Result<(), String> {
    // Deliberately no `cmd /C mklink /J` fallback: cmd.exe interprets
    // metacharacters (e.g. `&`) in arguments, so a hostile directory name
    // would become command injection. Directory symlinks require Developer
    // Mode or elevation on Windows; surface that instead.
    std::os::windows::fs::symlink_dir(target, link).map_err(|e| {
        format!(
            "failed to link {} -> {}: {e} (creating symlinks on Windows requires Developer Mode or an elevated process)",
            link.display(),
            target.display()
        )
    })
}

/// Link every folder in `folders` into `dest_root`, returning the created link paths.
fn link_folders_into(dest_root: &Path, folders: &[String]) -> Result<Vec<String>, String> {
    let canonical_dest = dest_root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dest_root.display()))?;
    let mut created = Vec::with_capacity(folders.len());
    for folder in folders {
        validate_path(folder)?;
        let source = Path::new(folder);
        if !source.is_dir() {
            return Err(format!("{folder}: not a directory"));
        }
        let canonical_source = source.canonicalize().map_err(|e| format!("{folder}: {e}"))?;
        if canonical_dest.starts_with(&canonical_source) {
            return Err(format!(
                "{folder}: linking a folder into itself or a descendant would create a cycle"
            ));
        }
        let link = unique_path(dest_root, &basename(&canonical_source));
        make_link(&simplified(&canonical_source), &link)?;
        created.push(link.to_string_lossy().into_owned());
    }
    Ok(created)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn create_folder_group(name: Option<String>, folders: Vec<String>) -> Result<String, String> {
    if folders.is_empty() {
        return Err("no folders selected".to_string());
    }
    let home = dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
    let base = home.join("sidex-groups");
    std::fs::create_dir_all(&base).map_err(|e| format!("{}: {e}", base.display()))?;
    let group_name = sanitize_name(&name.unwrap_or_else(|| default_group_name(&folders)));
    let group_dir = unique_path(&base, &group_name);
    std::fs::create_dir(&group_dir).map_err(|e| format!("{}: {e}", group_dir.display()))?;
    if let Err(e) = link_folders_into(&group_dir, &folders) {
        // Don't leave a partial group behind; it only contains links we just
        // created, so removing it cannot touch the linked folders' contents.
        let _ = std::fs::remove_dir_all(&group_dir);
        return Err(e);
    }
    Ok(group_dir.to_string_lossy().into_owned())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn add_folder_links(dest_root: String, folders: Vec<String>) -> Result<Vec<String>, String> {
    validate_path(&dest_root)?;
    let dest = Path::new(&dest_root);
    if !dest.is_dir() {
        return Err(format!("{dest_root}: not a directory"));
    }
    link_folders_into(dest, &folders)
}
