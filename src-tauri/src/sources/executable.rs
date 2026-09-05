use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};
use tokio::process::Command;

#[cfg(windows)]
fn registered_paths() -> Vec<OsString> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER, enums::HKEY_LOCAL_MACHINE};

    [
        (HKEY_CURRENT_USER, "Environment"),
        (
            HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
        ),
    ]
    .into_iter()
    .filter_map(|(root, key)| {
        RegKey::predef(root)
            .open_subkey(key)
            .ok()?
            .get_value::<String, _>("Path")
            .ok()
            .map(OsString::from)
    })
    .collect()
}

#[cfg(not(windows))]
fn registered_paths() -> Vec<OsString> {
    Vec::new()
}

fn search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for value in env::var_os("PATH").into_iter().chain(registered_paths()) {
        for path in env::split_paths(&value) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

#[cfg(windows)]
fn executable_extensions() -> Vec<OsString> {
    let value = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    value
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(OsString::from)
        .collect()
}

#[cfg(not(windows))]
fn executable_extensions() -> Vec<OsString> {
    Vec::new()
}

fn resolve_in(executable: &OsStr, paths: &[PathBuf], extensions: &[OsString]) -> Option<PathBuf> {
    let requested = Path::new(executable);
    if requested.components().count() != 1 {
        return None;
    }
    for directory in paths {
        let candidate = directory.join(requested);
        if candidate.is_file() {
            return Some(candidate);
        }
        if requested.extension().is_none() {
            for extension in extensions {
                let mut name = executable.to_os_string();
                name.push(extension);
                let candidate = directory.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

pub fn command(executable: &str) -> Command {
    let paths = search_paths();
    let program = resolve_in(OsStr::new(executable), &paths, &executable_extensions())
        .unwrap_or_else(|| PathBuf::from(executable));
    let mut command = Command::new(program);
    if let Ok(path) = env::join_paths(paths) {
        command.env("PATH", path);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolver_uses_fresh_paths_and_preserves_precedence() {
        let root = tempfile::tempdir().unwrap();
        let inherited = root.path().join("inherited");
        let current = root.path().join("current");
        fs::create_dir_all(&inherited).unwrap();
        fs::create_dir_all(&current).unwrap();
        fs::write(current.join("tool.exe"), b"current").unwrap();
        let paths = vec![inherited.clone(), current.clone()];
        let extensions = vec![OsString::from(".exe")];
        assert_eq!(
            resolve_in(OsStr::new("tool"), &paths, &extensions),
            Some(current.join("tool.exe"))
        );
        fs::write(inherited.join("tool.exe"), b"inherited").unwrap();
        assert_eq!(
            resolve_in(OsStr::new("tool"), &paths, &extensions),
            Some(inherited.join("tool.exe"))
        );
    }

    #[test]
    fn explicit_paths_are_not_reinterpreted_as_search_names() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_in(
                root.path().join("tool").as_os_str(),
                &[root.path().to_owned()],
                &[]
            ),
            None
        );
    }
}
