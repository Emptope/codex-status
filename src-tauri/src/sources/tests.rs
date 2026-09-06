use super::executable::marked_path;
use super::{
    drain,
    executable::{application_dirs_in, combine_paths, extend_paths, resolve_in},
    local::Local,
};
use crate::status::Activity;
use notify::{Event, EventKind, event::Flag};
use serde_json::json;
use std::{
    env,
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[test]
fn discovered_paths_extend_sparse_gui_environments() {
    let root = tempfile::tempdir().unwrap();
    let system = root.path().join("system");
    let user = root.path().join("user");
    fs::create_dir_all(&system).unwrap();
    fs::create_dir_all(&user).unwrap();
    fs::write(user.join("tool"), b"tool").unwrap();

    let mut paths = Vec::new();
    extend_paths(
        &mut paths,
        env::join_paths([system, user.clone()]).unwrap().as_os_str(),
    );
    assert_eq!(
        resolve_in(OsStr::new("tool"), &paths, &[]),
        Some(user.join("tool"))
    );
}

#[test]
fn discovered_paths_preserve_the_user_environment_precedence() {
    let root = tempfile::tempdir().unwrap();
    let inherited = root.path().join("inherited");
    let discovered = root.path().join("discovered");
    fs::create_dir_all(&inherited).unwrap();
    fs::create_dir_all(&discovered).unwrap();
    fs::write(inherited.join("tool"), b"inherited").unwrap();
    fs::write(discovered.join("tool"), b"discovered").unwrap();

    let paths = combine_paths(vec![discovered.clone()], vec![inherited]);

    assert_eq!(
        resolve_in(OsStr::new("tool"), &paths, &[]),
        Some(discovered.join("tool"))
    );
}

#[test]
fn application_resources_are_executable_search_candidates() {
    let root = tempfile::tempdir().unwrap();
    let resources = root
        .path()
        .join("Desktop.app")
        .join("Contents")
        .join("Resources");
    fs::create_dir_all(&resources).unwrap();
    fs::write(resources.join("tool"), b"tool").unwrap();

    let paths = application_dirs_in(OsStr::new("tool"), &[root.path().to_owned()]);

    assert_eq!(
        resolve_in(OsStr::new("tool"), &paths, &[]),
        Some(resources.join("tool"))
    );
}

#[test]
fn shell_path_marker_ignores_startup_output() {
    assert_eq!(
        marked_path(b"startup output\n\0/system:/user\0trailing\n"),
        Some(b"/system:/user".as_slice())
    );
    assert_eq!(marked_path(b"unmarked output"), None);
}

#[test]
fn dropped_and_imprecise_events_request_immediate_reconciliation() {
    let (send, receive) = mpsc::channel();
    send.send(Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)))
        .unwrap();
    let lost = AtomicBool::new(false);
    assert!(drain(&receive, &lost).reconcile);

    lost.store(true, Ordering::Relaxed);
    assert!(drain(&receive, &lost).reconcile);
    assert!(!lost.load(Ordering::Relaxed));
}

#[test]
fn reconciliation_advances_a_record_after_an_imprecise_change() {
    let directory = tempfile::tempdir().unwrap();
    let sessions = directory.path().join("sessions");
    let work = directory.path().join("work");
    fs::create_dir(&sessions).unwrap();
    fs::create_dir(&work).unwrap();
    let record = sessions.join("record");
    let meta = json!({"type":"session_meta","payload":{"id":"id","cwd":work}});
    let started = json!({"timestamp":"2026-09-05T00:00:01Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn"}});
    let completed = json!({"timestamp":"2026-09-05T00:00:02Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn"}});
    fs::write(&record, format!("{meta}\n{started}\n")).unwrap();
    let mut local = Local::default();
    let root = directory.path().to_string_lossy();
    local.update(&record, &root);
    OpenOptions::new()
        .append(true)
        .open(&record)
        .unwrap()
        .write_all(format!("{completed}\n").as_bytes())
        .unwrap();

    local.reconcile(&[root.into_owned()], || false);

    assert_eq!(
        local.snapshot()[0].activity.value,
        Some(Activity::Completed)
    );
}
