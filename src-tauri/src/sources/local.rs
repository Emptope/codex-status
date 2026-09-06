use super::{identity, text};
use crate::status::{Activity, Event, Field, Quality, Quota, QuotaWindow, Session, Usage};
use chrono::DateTime;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};
use walkdir::WalkDir;

const CHUNK: usize = 256 * 1024;
const MAX_LINE: usize = 1024 * 1024;
const RETAINED_LINE: usize = 16 * 1024;

fn at(row: &Value) -> Option<i64> {
    row["timestamp"]
        .as_str()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp_millis())
}

fn usage(value: &Value) -> Option<Usage> {
    value.as_object()?;
    Some(Usage {
        input: value["input_tokens"].as_u64(),
        cached_input: value["cached_input_tokens"].as_u64(),
        output: value["output_tokens"].as_u64(),
        total: value["total_tokens"].as_u64(),
    })
}

fn quota(value: &Value, observed_at: i64) -> Option<Quota> {
    value.as_object()?;
    let id = text(&value["limit_id"], 128)?;
    let windows = ["primary", "secondary"]
        .into_iter()
        .filter_map(|key| {
            let window = &value[key];
            window.as_object()?;
            let mut remaining = Field::absent("local", Quality::Unavailable);
            if let Some(used) = window["used_percent"]
                .as_f64()
                .filter(|used| used.is_finite() && (0.0..=100.0).contains(used))
            {
                remaining.set(100.0 - used, observed_at);
            }
            let resets_at = window["resets_at"]
                .as_i64()
                .and_then(|seconds| seconds.checked_mul(1000));
            if resets_at.is_some_and(|reset| reset <= observed_at) && remaining.value.is_some() {
                remaining.quality = Quality::Stale;
            }
            Some(QuotaWindow {
                remaining,
                minutes: window["window_minutes"]
                    .as_u64()
                    .filter(|minutes| *minutes > 0),
                resets_at,
            })
        })
        .collect();
    Some(Quota {
        name: text(&value["limit_name"], 128).unwrap_or_else(|| id.clone()),
        id,
        windows,
        credit_balance: text(&value["credits"]["balance"], 64),
        unlimited_credits: value["credits"]["unlimited"].as_bool(),
    })
}

fn rate_limits(row: &Value) -> Option<(i64, Quota)> {
    if row["type"] != "event_msg" || row["payload"]["type"] != "token_count" {
        return None;
    }
    let observed_at = at(row)?;
    quota(&row["payload"]["rate_limits"], observed_at).map(|quota| (observed_at, quota))
}

fn session(root: &str, row: &Value) -> Option<Session> {
    if row["type"] != "session_meta" {
        return None;
    }
    let payload = &row["payload"];
    let session_id = text(&payload["id"], 256)?;
    let path = text(&payload["cwd"], 4096).unwrap_or_default();
    let project = Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    let root_id = identity(root);
    Some(Session::new(
        format!("{root_id}:{session_id}"),
        path,
        project,
    ))
}

fn approval_argument(value: &Value) -> bool {
    if value["sandbox_permissions"] == "require_escalated" {
        return true;
    }
    let Some(source) = value.as_str() else {
        return false;
    };
    if serde_json::from_str::<Value>(source)
        .ok()
        .is_some_and(|arguments| arguments["sandbox_permissions"] == "require_escalated")
    {
        return true;
    }
    const CALL: &str = "tools.exec_command(";
    source.match_indices(CALL).any(|(index, _)| {
        let mut deserializer = serde_json::Deserializer::from_str(&source[index + CALL.len()..]);
        Value::deserialize(&mut deserializer)
            .ok()
            .is_some_and(|arguments| arguments["sandbox_permissions"] == "require_escalated")
    })
}

fn approval_request(payload: &Value) -> Option<String> {
    let supported = match payload["type"].as_str()? {
        "custom_tool_call" => payload["name"] == "exec",
        "function_call" => matches!(payload["name"].as_str(), Some("exec_command" | "exec")),
        _ => false,
    };
    (supported
        && (approval_argument(&payload["input"]) || approval_argument(&payload["arguments"])))
    .then(|| text(&payload["call_id"], 256))
    .flatten()
}

fn approval_response(payload: &Value) -> Option<String> {
    matches!(
        payload["type"].as_str(),
        Some("custom_tool_call_output" | "function_call_output")
    )
    .then(|| text(&payload["call_id"], 256))
    .flatten()
}

fn event(row: &Value) -> Option<Event> {
    let at = at(row)?;
    let payload = &row["payload"];
    match row["type"].as_str()? {
        "turn_context" => Some(Event::Context {
            at,
            model: text(&payload["model"], 128),
            effort: text(&payload["effort"], 32),
        }),
        "event_msg" => match payload["type"].as_str()? {
            "token_count" => Some(Event::Usage {
                at,
                total: usage(&payload["info"]["total_token_usage"]),
                last: usage(&payload["info"]["last_token_usage"]),
                context_limit: payload["info"]["model_context_window"].as_u64(),
            }),
            "task_started" => Some(Event::TurnStarted {
                at,
                turn: text(&payload["turn_id"], 256)?,
            }),
            "task_complete" => Some(Event::TurnEnded {
                at,
                turn: text(&payload["turn_id"], 256)?,
                activity: Activity::Completed,
                duration_ms: payload["duration_ms"].as_u64(),
            }),
            "turn_aborted" => Some(Event::TurnEnded {
                at,
                turn: text(&payload["turn_id"], 256)?,
                activity: Activity::Interrupted,
                duration_ms: payload["duration_ms"].as_u64(),
            }),
            _ => None,
        },
        "response_item" => approval_request(payload)
            .map(|request| Event::ApprovalRequested { at, request })
            .or_else(|| {
                approval_response(payload).map(|request| Event::ApprovalResolved { at, request })
            }),
        _ => None,
    }
}

#[derive(Default)]
pub struct Cursor {
    offset: u64,
    partial: Vec<u8>,
    discarding: bool,
    identity: Option<String>,
    modified: Option<SystemTime>,
    prefix: Vec<u8>,
}

impl Cursor {
    fn clear_partial(&mut self) {
        self.partial.clear();
        if self.partial.capacity() > RETAINED_LINE {
            self.partial.shrink_to(RETAINED_LINE);
        }
    }

    pub fn read(&mut self, path: &Path) -> std::io::Result<(Vec<Value>, bool, bool)> {
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            format!("{}:{}", metadata.dev(), metadata.ino())
        };
        #[cfg(not(unix))]
        let identity = format!("{:?}", metadata.created());
        let mut prefix = vec![0; self.prefix.len()];
        file.read_exact(&mut prefix).ok();
        let reset = self.identity.as_ref() != Some(&identity)
            || metadata.len() < self.offset
            || prefix != self.prefix
            || (metadata.len() == self.offset
                && self.modified.is_some()
                && metadata.modified().ok() != self.modified);
        if reset {
            self.offset = 0;
            self.partial = Vec::new();
            self.discarding = false;
            self.prefix.clear();
        }
        self.identity = Some(identity);
        self.modified = metadata.modified().ok();
        if self.prefix.is_empty() && metadata.len() > 0 {
            file.seek(SeekFrom::Start(0))?;
            self.prefix.resize(metadata.len().min(256) as usize, 0);
            file.read_exact(&mut self.prefix)?;
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = vec![0; CHUNK];
        let count = file.read(&mut bytes)?;
        self.offset += count as u64;
        let mut rows = Vec::new();
        for byte in &bytes[..count] {
            if *byte == b'\n' {
                if !self.discarding
                    && let Ok(row) = serde_json::from_slice(&self.partial)
                {
                    rows.push(row);
                }
                self.clear_partial();
                self.discarding = false;
            } else if !self.discarding {
                if self.partial.len() >= MAX_LINE {
                    self.clear_partial();
                    self.discarding = true;
                } else {
                    self.partial.push(*byte);
                }
            }
        }
        Ok((rows, reset, self.offset < metadata.len()))
    }
}

#[derive(Default)]
pub struct Local {
    cursors: BTreeMap<PathBuf, Cursor>,
    sessions: BTreeMap<PathBuf, Session>,
    quotas: BTreeMap<(PathBuf, String), Limit>,
    modified: BTreeMap<PathBuf, (u64, Option<SystemTime>)>,
    pub error: Option<String>,
}

struct Limit {
    root: String,
    observed_at: i64,
    quota: Quota,
}

impl Local {
    pub fn reconcile(&mut self, roots: &[String], cancelled: impl Fn() -> bool) {
        self.error = None;
        let mut files = Vec::new();
        for root in roots {
            if cancelled() {
                return;
            }
            let directory = Path::new(root).join("sessions");
            if !directory.is_dir() {
                self.error = Some("source-missing".into());
                continue;
            }
            for item in WalkDir::new(directory)
                .max_depth(8)
                .follow_links(false)
                .into_iter()
                .take(10001)
            {
                if cancelled() {
                    return;
                }
                let Ok(entry) = item else {
                    self.error = Some("source-unreadable".into());
                    continue;
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    files.push((meta.modified().ok(), entry.into_path(), root));
                }
            }
        }
        files.sort_by_key(|item| std::cmp::Reverse(item.0));
        for (_, path, root) in files.into_iter().take(50) {
            if cancelled() {
                return;
            }
            self.update_until(&path, root, &cancelled);
        }
        self.sessions.retain(|path, _| {
            path.exists()
                && roots
                    .iter()
                    .any(|root| path.starts_with(Path::new(root).join("sessions")))
        });
        self.cursors
            .retain(|path, _| self.sessions.contains_key(path));
        self.modified
            .retain(|path, _| self.sessions.contains_key(path));
        self.quotas.retain(|(path, _), _| {
            path.exists()
                && roots
                    .iter()
                    .any(|root| path.starts_with(Path::new(root).join("sessions")))
        });
    }

    #[cfg(test)]
    pub fn update(&mut self, path: &Path, root: &str) {
        self.update_until(path, root, &|| false);
    }

    pub fn update_until(&mut self, path: &Path, root: &str, cancelled: &impl Fn() -> bool) {
        if cancelled() {
            return;
        }
        let Ok(meta) = fs::symlink_metadata(path) else {
            self.sessions.remove(path);
            self.cursors.remove(path);
            self.quotas.retain(|(source, _), _| source != path);
            self.modified.remove(path);
            return;
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            return;
        }
        let signature = (meta.len(), meta.modified().ok());
        if self.modified.get(path) == Some(&signature) {
            return;
        }
        if self.cursors.len() >= 500 && !self.cursors.contains_key(path) {
            self.error = Some("source-limit".into());
            return;
        }
        let cursor = self.cursors.entry(path.to_owned()).or_default();
        for _ in 0..128 {
            if cancelled() {
                return;
            }
            match cursor.read(path) {
                Ok((rows, reset, more)) => {
                    if reset {
                        self.sessions.remove(path);
                        self.quotas.retain(|(source, _), _| source != path);
                    }
                    for row in rows {
                        if !self.sessions.contains_key(path)
                            && let Some(session) = session(root, &row)
                        {
                            self.sessions.insert(path.to_owned(), session);
                        }
                        if let (Some(session), Some(event)) =
                            (self.sessions.get_mut(path), event(&row))
                        {
                            session.apply(event);
                        }
                        if let Some((observed_at, quota)) = rate_limits(&row) {
                            let key = (path.to_owned(), quota.id.clone());
                            if self
                                .quotas
                                .get(&key)
                                .is_none_or(|current| observed_at >= current.observed_at)
                            {
                                self.quotas.insert(
                                    key,
                                    Limit {
                                        root: root.to_owned(),
                                        observed_at,
                                        quota,
                                    },
                                );
                            }
                        }
                    }
                    if !more {
                        self.modified.insert(path.to_owned(), signature);
                        return;
                    }
                }
                Err(_) => {
                    self.error = Some("record-unreadable".into());
                    return;
                }
            }
        }
        self.error = Some("record-limit".into());
    }

    pub fn snapshot(&self) -> Vec<Session> {
        let mut sessions: Vec<&Session> = self
            .sessions
            .values()
            .filter(|session| {
                let path = Path::new(&session.path);
                // Paths owned by another configured runtime cannot be verified on this host.
                !path.is_absolute() || path.is_dir()
            })
            .collect();
        sessions.sort_by(|a, b| b.latest_at.cmp(&a.latest_at).then_with(|| a.id.cmp(&b.id)));
        sessions.truncate(50);
        sessions.into_iter().cloned().collect()
    }

    pub fn quotas(&self, root: Option<&str>) -> (Vec<Quota>, Option<i64>) {
        let Some(root) = root else {
            return (Vec::new(), None);
        };
        let mut latest: BTreeMap<String, &Limit> = BTreeMap::new();
        for limit in self.quotas.values().filter(|limit| limit.root == root) {
            let current = latest.entry(limit.quota.id.clone()).or_insert(limit);
            if limit.observed_at > current.observed_at {
                *current = limit;
            }
        }
        let observed_at = latest.values().map(|limit| limit.observed_at).max();
        (
            latest
                .into_values()
                .map(|limit| limit.quota.clone())
                .collect(),
            observed_at,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    #[test]
    fn cursor_retains_partial_utf8_and_resets_on_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record");
        fs::write(&path, b"{\"a\":").unwrap();
        let mut cursor = Cursor::default();
        assert!(cursor.read(&path).unwrap().0.is_empty());
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"1}\nbroken\n{}\n")
            .unwrap();
        assert_eq!(cursor.read(&path).unwrap().0.len(), 2);
        assert!(cursor.read(&path).unwrap().0.is_empty());
        fs::write(&path, b"{}\n").unwrap();
        let (rows, reset, _) = cursor.read(&path).unwrap();
        assert!(reset);
        assert_eq!(rows.len(), 1);
        let new = dir.path().join("new");
        fs::write(&new, b"{\"b\":2}\n").unwrap();
        fs::rename(new, &path).unwrap();
        assert!(cursor.read(&path).unwrap().1);
    }

    #[test]
    fn cursor_releases_completed_large_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record");
        let payload = "a".repeat(RETAINED_LINE * 8);
        fs::write(&path, format!("{{\"ignored\":\"{payload}\"}}\n")).unwrap();

        let mut cursor = Cursor::default();
        assert_eq!(cursor.read(&path).unwrap().0.len(), 1);
        assert!(cursor.partial.capacity() <= RETAINED_LINE);
    }

    #[test]
    fn session_metadata_has_no_version_gate() {
        let row = json!({"type":"session_meta","payload":{"id":"id","cwd":"/work"}});
        let parsed = session("/data", &row).expect("session metadata should be accepted");

        assert_eq!(parsed.path, "/work");
    }

    #[test]
    fn command_approval_calls_and_outputs_become_session_events() {
        let approval = json!({
            "timestamp":"2026-09-05T00:00:02Z",
            "type":"response_item",
            "payload":{
                "type":"custom_tool_call",
                "name":"exec",
                "call_id":"approval",
                "input":"const result = await tools.exec_command({\"cmd\":\"build\",\"sandbox_permissions\":\"require_escalated\"});"
            }
        });
        assert_eq!(
            event(&approval),
            Some(Event::ApprovalRequested {
                at: 1_788_566_402_000,
                request: "approval".into(),
            })
        );

        let legacy = json!({
            "timestamp":"2026-09-05T00:00:03Z",
            "type":"response_item",
            "payload":{
                "type":"function_call",
                "name":"exec_command",
                "call_id":"legacy",
                "arguments":"{\"cmd\":\"build\",\"sandbox_permissions\":\"require_escalated\"}"
            }
        });
        assert!(matches!(
            event(&legacy),
            Some(Event::ApprovalRequested { request, .. }) if request == "legacy"
        ));

        let resolved = json!({
            "timestamp":"2026-09-05T00:00:04Z",
            "type":"response_item",
            "payload":{"type":"custom_tool_call_output","call_id":"approval"}
        });
        assert!(matches!(
            event(&resolved),
            Some(Event::ApprovalResolved { request, .. }) if request == "approval"
        ));
    }

    #[test]
    fn command_text_that_mentions_escalation_is_not_an_approval_request() {
        let row = json!({
            "timestamp":"2026-09-05T00:00:02Z",
            "type":"response_item",
            "payload":{
                "type":"custom_tool_call",
                "name":"exec",
                "call_id":"search",
                "input":"const result = await tools.exec_command({\"cmd\":\"rg require_escalated .\"});"
            }
        });
        assert_eq!(event(&row), None);
    }

    #[test]
    fn adapter_separates_roots_and_ignores_unverified_statuses() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        let one = directory.path().join("work/one");
        let two = directory.path().join("work/two");
        fs::create_dir_all(&one).unwrap();
        fs::create_dir_all(&two).unwrap();
        let meta = |path: &str| json!({"type":"session_meta","payload":{"id":"same","cwd":path,"instructions":"private"}});
        let unknown = json!({"timestamp":"2026-09-05T00:00:01Z","type":"event_msg","payload":{"type":"unverified_status","status":"failed"}});
        fs::write(
            &first,
            format!("{}\n{}\n", meta(&one.to_string_lossy()), unknown),
        )
        .unwrap();
        fs::write(&second, format!("{}\n", meta(&two.to_string_lossy()))).unwrap();
        let mut local = Local::default();
        local.update(&first, "/data/first");
        local.update(&second, "/data/second");
        let sessions = local.snapshot();
        assert_eq!(sessions.len(), 2);
        assert_ne!(sessions[0].id, sessions[1].id);
        assert!(
            sessions
                .iter()
                .all(|session| session.activity.value.is_none())
        );
        assert!(
            !serde_json::to_string(&sessions)
                .unwrap()
                .contains("private")
        );
    }

    #[test]
    fn snapshot_excludes_missing_local_working_directories() {
        let root = tempfile::tempdir().unwrap();
        let records = root.path().join("sessions");
        let available = root.path().join("available");
        let missing = root.path().join("missing");
        fs::create_dir(&records).unwrap();
        fs::create_dir(&available).unwrap();
        let meta =
            |id: &str, path: &Path| json!({"type":"session_meta","payload":{"id":id,"cwd":path}});
        let available_record = records.join("available.jsonl");
        let missing_record = records.join("missing.jsonl");
        fs::write(
            &available_record,
            format!("{}\n", meta("available", &available)),
        )
        .unwrap();
        fs::write(&missing_record, format!("{}\n", meta("missing", &missing))).unwrap();

        let mut local = Local::default();
        let root = root.path().to_string_lossy();
        local.update(&available_record, &root);
        local.update(&missing_record, &root);
        assert_eq!(local.snapshot().len(), 1);
        assert_eq!(local.snapshot()[0].path, available.to_string_lossy());

        fs::remove_dir(available).unwrap();
        assert!(local.snapshot().is_empty());
    }

    #[test]
    fn reconciliation_and_reading_stop_when_cancelled() {
        let directory = tempfile::tempdir().unwrap();
        let sessions = directory.path().join("sessions");
        fs::create_dir(&sessions).unwrap();
        fs::write(sessions.join("record"), b"{}\n").unwrap();
        let mut local = Local::default();
        let roots = vec![directory.path().to_string_lossy().into_owned()];
        local.reconcile(&roots, || true);
        local.update_until(&sessions.join("record"), &roots[0], &|| true);
        assert!(local.sessions.is_empty());
        assert!(local.cursors.is_empty());
        assert!(local.error.is_none());
    }

    #[test]
    fn adapter_keeps_latest_rate_limits_within_the_selected_root() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        let meta = json!({"type":"session_meta","payload":{"id":"id","cwd":"/work"}});
        let current = json!({"timestamp":"2026-09-05T00:00:02Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":75,"window_minutes":300,"resets_at":200},"secondary":null,"credits":{"balance":"4","unlimited":false}}}});
        let old = json!({"timestamp":"2026-09-05T00:00:01Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":5,"window_minutes":60,"resets_at":100}}}});
        let other = json!({"timestamp":"2026-09-05T00:00:03Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":10,"window_minutes":30,"resets_at":300}}}});
        fs::write(&first, format!("{meta}\n{current}\n{old}\n")).unwrap();
        fs::write(&second, format!("{meta}\n{other}\n")).unwrap();

        let mut local = Local::default();
        local.update(&first, "/data/first");
        local.update(&second, "/data/second");
        let (quotas, observed_at) = local.quotas(Some("/data/first"));

        assert_eq!(observed_at, Some(1_788_566_402_000));
        assert_eq!(quotas.len(), 1);
        assert_eq!(quotas[0].name, "codex");
        assert_eq!(quotas[0].windows.len(), 1);
        assert_eq!(quotas[0].windows[0].remaining.value, Some(25.0));
        assert_eq!(quotas[0].windows[0].remaining.source, "local");
        assert_eq!(quotas[0].windows[0].minutes, Some(300));
        assert_eq!(quotas[0].windows[0].resets_at, Some(200_000));
        assert_eq!(quotas[0].credit_balance.as_deref(), Some("4"));
        assert_eq!(quotas[0].unlimited_credits, Some(false));
    }
}
