use super::executable::marked_path;
use super::{
    QueryWake, RuntimeState, drain,
    executable::{application_dirs_in, combine_paths, extend_paths, resolve_in},
    local::Local,
    local_quota_advanced, quota_refresh_throttle,
};
use crate::status::{Activity, Field, Quality, Quota, QuotaWindow, Session, Snapshot};
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
    time::Duration,
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
fn newer_local_quota_events_wake_the_authoritative_rpc_source_once() {
    let mut observed_at = None;

    assert!(!local_quota_advanced(
        super::QuotaSource::Rpc,
        None,
        &mut observed_at
    ));
    assert!(local_quota_advanced(
        super::QuotaSource::Rpc,
        Some(100),
        &mut observed_at
    ));
    assert!(!local_quota_advanced(
        super::QuotaSource::Rpc,
        Some(100),
        &mut observed_at
    ));
    assert!(!local_quota_advanced(
        super::QuotaSource::Local,
        Some(200),
        &mut observed_at
    ));
    assert!(!local_quota_advanced(
        super::QuotaSource::Rpc,
        Some(200),
        &mut observed_at
    ));
    assert!(local_quota_advanced(
        super::QuotaSource::Rpc,
        Some(300),
        &mut observed_at
    ));
}

#[test]
fn manual_refresh_bypasses_the_automatic_quota_throttle() {
    let recent = Duration::from_millis(100);

    assert_eq!(
        quota_refresh_throttle(QueryWake::Manual, recent),
        Duration::ZERO
    );
    assert_eq!(
        quota_refresh_throttle(QueryWake::Scheduled, recent),
        Duration::ZERO
    );
    assert_eq!(
        quota_refresh_throttle(QueryWake::Automatic, recent),
        Duration::from_millis(2900)
    );
}

#[test]
fn tracked_refresh_advances_a_record_after_a_missed_change_event() {
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
    let encoded = completed.to_string();
    let encoded = encoded.as_bytes();
    let split = encoded.len() / 2;
    let mut file = OpenOptions::new().append(true).open(&record).unwrap();
    file.write_all(&encoded[..split]).unwrap();
    local.update(&record, &root);
    assert_eq!(local.snapshot()[0].activity.value, Some(Activity::Running));
    file.write_all(&encoded[split..]).unwrap();
    file.write_all(b"\n").unwrap();

    let roots = [root.into_owned()];
    assert!(local.refresh_tracked(&roots, &|| false));

    assert_eq!(
        local.snapshot()[0].activity.value,
        Some(Activity::Completed)
    );
}

fn quota(source: &str, observed_at: i64, remaining: f64) -> Quota {
    let mut field = Field::absent(source, Quality::Unavailable);
    field.set(remaining, observed_at);
    Quota {
        id: "codex".into(),
        name: "Codex".into(),
        windows: vec![QuotaWindow {
            remaining: field,
            minutes: Some(300),
            resets_at: None,
        }],
        credit_balance: None,
        unlimited_credits: None,
    }
}

#[test]
fn each_account_mode_has_one_authoritative_quota_owner() {
    let mut state = RuntimeState::new(Snapshot::default());
    let rpc_owner = state
        .observe_account(0, "chatgpt", "account-a".into(), None, 1)
        .unwrap();
    assert!(state.observe_rpc(&rpc_owner, vec![quota("appServer", 100, 80.0)], 100));

    assert!(state.observe_local(
        0,
        Vec::new(),
        None,
        vec![quota("local", 200, 10.0)],
        Some(200),
    ));
    assert_eq!(
        state.snapshot.quotas[0].windows[0].remaining.source,
        "appServer"
    );
    assert_eq!(
        state.snapshot.quotas[0].windows[0].remaining.value,
        Some(80.0)
    );

    state
        .observe_account(0, "apiKey", "account-b".into(), None, 201)
        .unwrap();
    assert!(state.snapshot.quotas.is_empty());
    assert!(!state.observe_rpc(&rpc_owner, vec![quota("appServer", 202, 70.0)], 202));
    assert!(state.observe_local(
        0,
        Vec::new(),
        None,
        vec![quota("local", 50, 25.0)],
        Some(50),
    ));
    assert_eq!(
        state.snapshot.quotas[0].windows[0].remaining.source,
        "local"
    );

    state
        .observe_account(0, "signedOut", "signed-out".into(), None, 203)
        .unwrap();
    assert!(state.snapshot.quotas.is_empty());
}

#[test]
fn account_and_configuration_changes_reject_stale_quota_results() {
    let mut state = RuntimeState::new(Snapshot::default());
    let old_owner = state
        .observe_account(0, "chatgpt", "account-a".into(), None, 1)
        .unwrap();
    assert!(state.observe_rpc(&old_owner, vec![quota("appServer", 2, 90.0)], 2));

    let new_owner = state
        .observe_account(0, "chatgpt", "account-b".into(), None, 3)
        .unwrap();
    assert!(state.snapshot.quotas.is_empty());
    assert!(!state.observe_rpc(&old_owner, vec![quota("appServer", 4, 80.0)], 4));
    assert!(state.observe_rpc(&new_owner, vec![quota("appServer", 5, 70.0)], 5));

    state.reset_sources(false);
    assert_eq!(state.snapshot.account, "unknown");
    assert_eq!(state.snapshot.connection, "connecting");
    assert!(state.snapshot.quotas.is_empty());
    assert!(!state.observe_rpc(&new_owner, vec![quota("appServer", 6, 60.0)], 6));
    assert!(!state.observe_local(0, Vec::new(), None, vec![quota("local", 7, 50.0)], Some(7),));
}

#[test]
fn root_generation_change_clears_sessions_and_rejects_old_scan() {
    let mut state = RuntimeState::new(Snapshot::default());
    state
        .snapshot
        .sessions
        .push(Session::new("old".into(), "/old".into(), "old".into()));
    state.reset_sources(true);
    assert!(state.snapshot.sessions.is_empty());

    assert!(!state.observe_local(
        0,
        vec![Session::new(
            "stale".into(),
            "/stale".into(),
            "stale".into(),
        )],
        None,
        Vec::new(),
        None,
    ));
    assert!(state.snapshot.sessions.is_empty());
}
