mod config;
mod executable;
mod local;
#[cfg(test)]
#[path = "tests.rs"]
mod regression;
mod rpc;

use crate::{
    settings::Settings,
    status::{Quality, Snapshot},
};
use notify::{RecursiveMode, Watcher};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tokio::{sync::Notify, task::JoinHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaSource {
    Rpc,
    Local,
    None,
}

#[derive(Default)]
struct Changes {
    paths: BTreeSet<PathBuf>,
    reconcile: bool,
}

impl Changes {
    fn push(&mut self, event: notify::Result<notify::Event>) {
        match event {
            Ok(event) => {
                self.reconcile |= event.need_rescan();
                self.paths.extend(event.paths);
            }
            Err(_) => self.reconcile = true,
        }
    }
}

fn drain(receive: &mpsc::Receiver<notify::Result<notify::Event>>, lost: &AtomicBool) -> Changes {
    let mut changes = Changes {
        reconcile: lost.swap(false, Ordering::Relaxed),
        ..Changes::default()
    };
    while let Ok(event) = receive.try_recv() {
        changes.push(event);
    }
    changes
}

fn quotas_observed_at(quotas: &[crate::status::Quota]) -> Option<i64> {
    quotas
        .iter()
        .flat_map(|quota| &quota.windows)
        .filter_map(|window| window.remaining.observed_at)
        .max()
}

fn prefer_local_quotas(source: QuotaSource, local: Option<i64>, current: Option<i64>) -> bool {
    source == QuotaSource::Local
        || source == QuotaSource::Rpc
            && local.is_some_and(|local| current.is_none_or(|current| local > current))
}

fn account_mode(value: &Value) -> &'static str {
    match value["account"]["type"].as_str() {
        Some("chatgpt") => "chatgpt",
        Some("apiKey") => "apiKey",
        Some(_) => "unsupported",
        None if value["requiresOpenaiAuth"] == false => "externalProvider",
        None => "signedOut",
    }
}

fn quota_source(mode: &str) -> QuotaSource {
    match mode {
        "chatgpt" => QuotaSource::Rpc,
        "apiKey" | "externalProvider" => QuotaSource::Local,
        _ => QuotaSource::None,
    }
}

fn identity(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..24].to_owned()
}

fn text(value: &Value, max: usize) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= max)
        .map(str::to_owned)
}

pub struct Runtime {
    pub settings: Mutex<Settings>,
    pub state: Mutex<Snapshot>,
    pub settings_path: PathBuf,
    pub hidden: AtomicBool,
    started: AtomicBool,
    stop: AtomicBool,
    refresh: Notify,
    shutdown: Notify,
    jobs: Mutex<Vec<JoinHandle<()>>>,
    changed: Box<dyn Fn(Snapshot) + Send + Sync>,
}

impl Runtime {
    pub fn new(path: PathBuf, changed: impl Fn(Snapshot) + Send + Sync + 'static) -> Arc<Self> {
        let (settings, warning) = Settings::load(&path);
        let state = Snapshot {
            error: warning,
            ..Snapshot::default()
        };
        Arc::new(Self {
            settings: Mutex::new(settings),
            state: Mutex::new(state),
            settings_path: path,
            hidden: AtomicBool::new(false),
            started: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            refresh: Notify::new(),
            shutdown: Notify::new(),
            jobs: Mutex::new(Vec::new()),
            changed: Box::new(changed),
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }
    pub fn preferences(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
    pub fn remember_position(&self, position: (i32, i32)) {
        self.settings.lock().unwrap().position = Some(position);
    }
    pub fn persist(&self) -> Result<(), String> {
        self.preferences().save(&self.settings_path)
    }
    pub fn request_refresh(&self) {
        self.refresh.notify_one();
    }

    pub fn update_settings(&self, settings: Settings) -> Result<(), String> {
        settings.save(&self.settings_path)?;
        let previous = self.preferences();
        let source_changed =
            previous.roots != settings.roots || previous.executable != settings.executable;
        *self.settings.lock().unwrap() = settings;
        if source_changed {
            self.request_refresh();
        }
        Ok(())
    }

    fn publish(&self, change: impl FnOnce(&mut Snapshot)) {
        let state = {
            let mut state = self.state.lock().unwrap();
            let previous = state.clone();
            change(&mut state);
            if *state == previous {
                return;
            }
            state.revision += 1;
            state.clone()
        };
        (self.changed)(state);
    }

    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::Relaxed) {
            return;
        }
        let local = self.clone();
        let account = self.clone();
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push(tokio::task::spawn_blocking(move || local.watch()));
        jobs.push(tokio::spawn(async move {
            account.query().await;
        }));
    }

    pub async fn close(&self) -> bool {
        self.stop.store(true, Ordering::Relaxed);
        self.shutdown.notify_waiters();
        let mut jobs = std::mem::take(&mut *self.jobs.lock().unwrap());
        let finished = tokio::time::timeout(Duration::from_secs(3), async {
            for job in &mut jobs {
                let _ = job.await;
            }
        })
        .await
        .is_ok();
        if !finished {
            for job in &jobs {
                job.abort();
            }
            tokio::task::yield_now().await;
        }
        finished
    }

    fn watch(&self) {
        let (send, receive) = mpsc::sync_channel(1024);
        let lost = Arc::new(AtomicBool::new(false));
        let callback_lost = lost.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if matches!(send.try_send(event), Err(mpsc::TrySendError::Full(_))) {
                    callback_lost.store(true, Ordering::Relaxed);
                }
            })
            .ok();
        let mut roots: Vec<String> = Vec::new();
        let mut local = local::Local::default();
        let mut reconcile = Instant::now() - Duration::from_secs(60);
        let mut source = QuotaSource::None;
        let mut pending = None;
        while !self.stop.load(Ordering::Relaxed) {
            let mut dirty = false;
            let preferences = self.preferences();
            if roots != preferences.roots {
                if let Some(watcher) = watcher.as_mut() {
                    for root in &roots {
                        let _ = watcher.unwatch(&Path::new(root).join("sessions"));
                    }
                    for root in &preferences.roots {
                        let _ = watcher
                            .watch(&Path::new(root).join("sessions"), RecursiveMode::Recursive);
                    }
                }
                roots = preferences.roots;
                local = local::Local::default();
                reconcile = Instant::now() - Duration::from_secs(60);
                dirty = true;
            }
            let next_source = {
                let state = self.state.lock().unwrap();
                quota_source(&state.account)
            };
            if source != next_source {
                source = next_source;
                dirty = true;
            }
            let mut changes = drain(&receive, &lost);
            if let Some(event) = pending.take() {
                changes.push(event);
            }
            changes.reconcile |= changes.paths.iter().any(|path| {
                path.is_dir()
                    || roots
                        .iter()
                        .any(|root| Path::new(root).join("sessions").as_path() == path.as_path())
            });
            dirty |= changes.reconcile || !changes.paths.is_empty();
            if changes.reconcile {
                local.reconcile(&roots, || self.stop.load(Ordering::Relaxed));
                reconcile = Instant::now();
            } else {
                for path in changes.paths {
                    if self.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Some(root) = roots
                        .iter()
                        .find(|root| path.starts_with(Path::new(root).join("sessions")))
                    {
                        local.update_until(&path, root, &|| self.stop.load(Ordering::Relaxed));
                    }
                }
            }
            if reconcile.elapsed() >= Duration::from_secs(60) {
                local.reconcile(&roots, || self.stop.load(Ordering::Relaxed));
                reconcile = Instant::now();
                dirty = true;
            }
            if dirty {
                let sessions = local.snapshot();
                let local_error = local.error.clone();
                let (quotas, observed_at) = local.quotas(roots.first().map(String::as_str));
                self.publish(move |state| {
                    state.sessions = sessions;
                    state.local_error = local_error;
                    if prefer_local_quotas(source, observed_at, quotas_observed_at(&state.quotas)) {
                        state.quotas = quotas;
                        if observed_at.is_some() {
                            state.updated_at = observed_at;
                        }
                    }
                });
            }
            pending = match receive.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(Duration::from_millis(250));
                    None
                }
            };
        }
    }

    async fn query(&self) {
        let mut client = None;
        let mut configuration = None;
        let mut failures: u32 = 0;
        let mut auth_paused = false;
        let mut last_query;
        loop {
            if self.stop.load(Ordering::Relaxed) {
                break;
            }
            let settings = self.preferences();
            let signature = (settings.executable.clone(), settings.roots.first().cloned());
            if configuration.as_ref() != Some(&signature) {
                if let Some(rpc) = client.take() {
                    rpc::Rpc::close(rpc).await;
                }
                self.publish(|state| {
                    state.quotas.clear();
                    state.account = "unknown".into();
                    state.provider = None;
                });
                configuration = Some(signature);
                failures = 0;
                auth_paused = false;
            }
            if auth_paused {
                tokio::select! {
                    _ = self.shutdown.notified() => break,
                    _ = self.refresh.notified() => auth_paused = false,
                }
                continue;
            }
            self.publish(|state| {
                state.refreshing = true;
                state.connection = "connecting".into();
            });
            let operation = async {
                if client.is_none() {
                    client = Some(
                        rpc::Rpc::start(
                            &settings.executable,
                            settings.roots.first().map(String::as_str),
                        )
                        .await?,
                    );
                }
                let rpc = client.as_mut().ok_or("source-unavailable")?;
                let account = rpc.call("account/read").await?;
                let mode = account_mode(&account);
                let quota_source = quota_source(mode);
                let provider = (mode == "externalProvider")
                    .then(|| config::provider(settings.roots.first().map(String::as_str)))
                    .flatten();
                self.publish(|state| {
                    state.account = mode.into();
                    state.provider = provider;
                });
                if quota_source != QuotaSource::Rpc {
                    self.publish(|state| {
                        if quota_source == QuotaSource::None {
                            state.quotas.clear();
                        }
                        state.connection = mode.into();
                        state.error = None;
                        if quota_source == QuotaSource::None {
                            state.updated_at = Some(chrono::Utc::now().timestamp_millis());
                        }
                    });
                    if mode == "signedOut" {
                        auth_paused = true;
                    }
                    return Ok::<(), String>(());
                }
                let limits = rpc.call("account/rateLimits/read").await?;
                let now = chrono::Utc::now().timestamp_millis();
                self.publish(|state| {
                    state.quotas = rpc::quotas(&limits, now);
                    state.connection = "connected".into();
                    state.error = None;
                    state.updated_at = Some(now);
                });
                Ok(())
            };
            let result = tokio::select! { result = operation => result, _ = self.shutdown.notified() => break };
            if let Err(error) = result {
                failures = failures.saturating_add(1);
                if error == "auth-required" {
                    auth_paused = true;
                }
                self.publish(|state| {
                    state.connection = "offline".into();
                    state.error = Some(error);
                    for bucket in &mut state.quotas {
                        for window in &mut bucket.windows {
                            if window.remaining.value.is_some() {
                                window.remaining.quality = Quality::Stale;
                            }
                        }
                    }
                });
                if let Some(rpc) = client.take() {
                    rpc.close().await;
                }
            } else {
                failures = 0;
            }
            self.publish(|state| {
                state.refreshing = false;
            });
            if auth_paused {
                continue;
            }
            let interval = if failures > 0 {
                (5_u64.saturating_mul(2_u64.saturating_pow(failures.min(6)))).min(300)
                    + (chrono::Utc::now().timestamp_subsec_millis() % 4) as u64
            } else if self.hidden.load(Ordering::Relaxed) {
                180
            } else {
                60
            };
            last_query = Instant::now();
            let manually_refreshed = tokio::select! {
                _ = self.shutdown.notified() => break,
                _ = self.refresh.notified() => true,
                _ = tokio::time::sleep(Duration::from_secs(interval)) => false,
            };
            if manually_refreshed {
                let throttle = Duration::from_secs(3).saturating_sub(last_query.elapsed());
                if !throttle.is_zero() {
                    tokio::select! {
                        _ = self.shutdown.notified() => break,
                        _ = tokio::time::sleep(throttle) => {}
                    }
                }
            }
            self.publish(|state| {
                let now = chrono::Utc::now().timestamp_millis();
                for bucket in &mut state.quotas {
                    for window in &mut bucket.windows {
                        if window.resets_at.is_some_and(|at| at <= now)
                            && window.remaining.value.is_some()
                        {
                            window.remaining.quality = Quality::Stale;
                        }
                    }
                }
            });
        }
        if let Some(rpc) = client {
            rpc.close().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn account_modes_choose_a_quota_source() {
        assert_eq!(quota_source("chatgpt"), QuotaSource::Rpc);
        assert_eq!(quota_source("apiKey"), QuotaSource::Local);
        assert_eq!(quota_source("externalProvider"), QuotaSource::Local);
        assert_eq!(quota_source("signedOut"), QuotaSource::None);
        assert_eq!(quota_source("unsupported"), QuotaSource::None);

        let external = json!({"account": null, "requiresOpenaiAuth": false});
        assert_eq!(account_mode(&external), "externalProvider");
    }
}
