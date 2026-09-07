use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};
#[cfg(target_os = "macos")]
use std::{process::Stdio, time::Duration};
use tokio::process::Command;
#[cfg(target_os = "macos")]
use tokio::time::timeout;

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

pub(super) fn extend_paths(paths: &mut Vec<PathBuf>, value: &OsStr) {
    for path in env::split_paths(value) {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
}

fn inherited_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for value in env::var_os("PATH").into_iter().chain(registered_paths()) {
        extend_paths(&mut paths, &value);
    }
    paths
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn combine_paths(mut primary: Vec<PathBuf>, fallback: Vec<PathBuf>) -> Vec<PathBuf> {
    for path in fallback {
        if !primary.contains(&path) {
            primary.push(path);
        }
    }
    primary
}

#[cfg(target_os = "macos")]
fn application_dirs(executable: &OsStr) -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    application_dirs_in(executable, &roots)
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn application_dirs_in(executable: &OsStr, roots: &[PathBuf]) -> Vec<PathBuf> {
    if Path::new(executable).components().count() != 1 {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut bundles: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
            .collect();
        bundles.sort();
        for bundle in bundles {
            let resources = bundle.join("Contents").join("Resources");
            if resources.join(executable).is_file() && !paths.contains(&resources) {
                paths.push(resources);
            }
        }
    }
    paths
}

#[cfg(target_os = "macos")]
async fn login_paths() -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStringExt;

    let shell = env::var_os("SHELL")
        .filter(|path| Path::new(path).is_absolute() && Path::new(path).is_file())
        .unwrap_or_else(|| OsString::from("/bin/zsh"));
    let mut command = Command::new(shell);
    command
        .args(["-ilc", r#"printf '\000%s\000' "$PATH""#])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let Ok(Ok(output)) = timeout(Duration::from_secs(5), command.output()).await else {
        return Vec::new();
    };
    let Some(value) = marked_path(&output.stdout) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    extend_paths(&mut paths, &OsString::from_vec(value.to_vec()));
    paths
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn marked_path(output: &[u8]) -> Option<&[u8]> {
    let start = output.iter().position(|byte| *byte == 0)? + 1;
    let end = output[start..].iter().position(|byte| *byte == 0)? + start;
    Some(&output[start..end])
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

pub(super) fn resolve_in(
    executable: &OsStr,
    paths: &[PathBuf],
    extensions: &[OsString],
) -> Option<PathBuf> {
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

fn same_file(left: &Path, right: &Path) -> bool {
    left.canonicalize()
        .ok()
        .zip(right.canonicalize().ok())
        .is_some_and(|(left, right)| left == right)
}

pub async fn command(executable: &str) -> Result<Command, String> {
    let inherited = inherited_paths();
    #[cfg(target_os = "macos")]
    let paths = combine_paths(
        combine_paths(login_paths().await, inherited),
        application_dirs(OsStr::new(executable)),
    );
    #[cfg(not(target_os = "macos"))]
    let paths = inherited;
    let program = resolve_in(OsStr::new(executable), &paths, &executable_extensions())
        .unwrap_or_else(|| PathBuf::from(executable));
    if env::current_exe().is_ok_and(|current| same_file(&program, &current)) {
        return Err("source-start-failed".into());
    }
    let mut command = Command::new(program);
    if let Ok(path) = env::join_paths(paths) {
        command.env("PATH", path);
    }
    Ok(command)
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

    #[tokio::test]
    async fn current_process_cannot_be_launched_as_its_own_source() {
        let current = env::current_exe().unwrap();
        let error = command(current.to_str().unwrap()).await.unwrap_err();
        assert_eq!(error, "source-start-failed");
    }
}
