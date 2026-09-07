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

const WATCH_WAIT: Duration = Duration::from_millis(250);
const TRACKED_REFRESH: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaSource {
    Rpc,
    Local,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryWake {
    Manual,
    Automatic,
    Scheduled,
}

fn quota_refresh_throttle(wake: QueryWake, elapsed: Duration) -> Duration {
    if wake == QueryWake::Automatic {
        Duration::from_secs(3).saturating_sub(elapsed)
    } else {
        Duration::ZERO
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotaOwner {
    generation: u64,
    source: QuotaSource,
    account: String,
}

struct RuntimeState {
    snapshot: Snapshot,
    generation: u64,
    quota_owner: Option<QuotaOwner>,
}

impl RuntimeState {
    fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            generation: 0,
            quota_owner: None,
        }
    }

    fn reset_sources(&mut self, roots_changed: bool) {
        self.generation = self.generation.wrapping_add(1);
        self.quota_owner = None;
        self.snapshot.quotas.clear();
        self.snapshot.account = "unknown".into();
        self.snapshot.provider = None;
        self.snapshot.updated_at = None;
        self.snapshot.error = None;
        self.snapshot.connection = "connecting".into();
        self.snapshot.refreshing = false;
        if roots_changed {
            self.snapshot.sessions.clear();
            self.snapshot.local_error = None;
        }
    }

    fn observe_account(
        &mut self,
        generation: u64,
        mode: &str,
        account: String,
        provider: Option<String>,
        at: i64,
    ) -> Option<QuotaOwner> {
        if generation != self.generation {
            return None;
        }
        let owner = QuotaOwner {
            generation,
            source: quota_source(mode),
            account,
        };
        if self.quota_owner.as_ref() != Some(&owner) {
            self.snapshot.quotas.clear();
            self.snapshot.updated_at = None;
        }
        self.quota_owner = Some(owner.clone());
        self.snapshot.account = mode.into();
        self.snapshot.provider = provider;
        self.snapshot.error = None;
        match owner.source {
            QuotaSource::Rpc => self.snapshot.connection = "connecting".into(),
            QuotaSource::Local => self.snapshot.connection = mode.into(),
            QuotaSource::None => {
                self.snapshot.connection = mode.into();
                self.snapshot.updated_at = Some(at);
            }
        }
        Some(owner)
    }

    fn observe_local(
        &mut self,
        generation: u64,
        sessions: Vec<crate::status::Session>,
        local_error: Option<String>,
        quotas: Vec<crate::status::Quota>,
        observed_at: Option<i64>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.snapshot.sessions = sessions;
        self.snapshot.local_error = local_error;
        if self.quota_owner.as_ref().is_some_and(|owner| {
            owner.generation == generation && owner.source == QuotaSource::Local
        }) {
            self.snapshot.quotas = quotas;
            if observed_at.is_some() {
                self.snapshot.updated_at = observed_at;
            }
        }
        true
    }

    fn observe_rpc(
        &mut self,
        owner: &QuotaOwner,
        quotas: Vec<crate::status::Quota>,
        at: i64,
    ) -> bool {
        if owner.source != QuotaSource::Rpc
            || owner.generation != self.generation
            || self.quota_owner.as_ref() != Some(owner)
        {
            return false;
        }
        self.snapshot.quotas = quotas;
        self.snapshot.connection = "connected".into();
        self.snapshot.error = None;
        self.snapshot.updated_at = Some(at);
        true
    }
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

fn local_quota_advanced(
    source: QuotaSource,
    observed_at: Option<i64>,
    previous: &mut Option<i64>,
) -> bool {
    let advanced = observed_at.is_some_and(|at| previous.is_none_or(|old| at > old));
    if advanced {
        *previous = observed_at;
    }
    advanced && source == QuotaSource::Rpc
}

fn identity(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..24].to_owned()
}

fn account_identity(value: &Value) -> String {
    identity(&serde_json::to_string(value).unwrap_or_default())
}

fn text(value: &Value, max: usize) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= max)
        .map(str::to_owned)
}

pub struct Runtime {
    pub settings: Mutex<Settings>,
    state: Mutex<RuntimeState>,
    pub settings_path: PathBuf,
    pub hidden: AtomicBool,
    started: AtomicBool,
    stop: AtomicBool,
    local_refresh: AtomicBool,
    refresh: Notify,
    quota_refresh: Notify,
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
            state: Mutex::new(RuntimeState::new(state)),
            settings_path: path,
            hidden: AtomicBool::new(false),
            started: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            local_refresh: AtomicBool::new(false),
            refresh: Notify::new(),
            quota_refresh: Notify::new(),
            shutdown: Notify::new(),
            jobs: Mutex::new(Vec::new()),
            changed: Box::new(changed),
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().snapshot.clone()
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
        self.local_refresh.store(true, Ordering::Release);
        self.refresh.notify_one();
    }

    fn request_quota_refresh(&self) {
        self.quota_refresh.notify_one();
    }

    pub fn update_settings(&self, settings: Settings) -> Result<(), String> {
        settings.save(&self.settings_path)?;
        let (source_changed, state) = {
            let mut preferences = self.settings.lock().unwrap();
            let roots_changed = preferences.roots != settings.roots;
            let source_changed = roots_changed || preferences.executable != settings.executable;
            *preferences = settings;
            let state = source_changed.then(|| {
                let mut state = self.state.lock().unwrap();
                let previous = state.snapshot.clone();
                state.reset_sources(roots_changed);
                Self::finish_change(&mut state.snapshot, previous)
            });
            (source_changed, state.flatten())
        };
        if let Some(state) = state {
            (self.changed)(state);
        }
        if source_changed {
            self.request_refresh();
        }
        Ok(())
    }

    fn finish_change(state: &mut Snapshot, previous: Snapshot) -> Option<Snapshot> {
        if *state == previous {
            return None;
        }
        state.revision += 1;
        Some(state.clone())
    }

    fn update_state<T>(&self, change: impl FnOnce(&mut RuntimeState) -> T) -> T {
        let (result, changed) = {
            let mut state = self.state.lock().unwrap();
            let previous = state.snapshot.clone();
            let result = change(&mut state);
            let changed = Self::finish_change(&mut state.snapshot, previous);
            (result, changed)
        };
        if let Some(state) = changed {
            (self.changed)(state);
        }
        result
    }

    fn source_configuration(&self) -> (Settings, u64) {
        let settings = self.settings.lock().unwrap().clone();
        let generation = self.state.lock().unwrap().generation;
        (settings, generation)
    }

    fn publish_generation(&self, generation: u64, change: impl FnOnce(&mut Snapshot)) {
        self.update_state(|state| {
            if state.generation == generation {
                change(&mut state.snapshot);
            }
        });
    }

    fn publish_account(
        &self,
        generation: u64,
        mode: &str,
        account: String,
        provider: Option<String>,
        at: i64,
    ) -> Option<QuotaOwner> {
        self.update_state(|state| state.observe_account(generation, mode, account, provider, at))
    }

    fn publish_local(
        &self,
        generation: u64,
        sessions: Vec<crate::status::Session>,
        local_error: Option<String>,
        quotas: Vec<crate::status::Quota>,
        observed_at: Option<i64>,
    ) {
        self.update_state(|state| {
            state.observe_local(generation, sessions, local_error, quotas, observed_at);
        });
    }

    fn publish_rpc(&self, owner: &QuotaOwner, quotas: Vec<crate::status::Quota>, at: i64) {
        self.update_state(|state| {
            state.observe_rpc(owner, quotas, at);
        });
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
        let mut tracked_refresh = Instant::now();
        let mut source = QuotaSource::None;
        let mut local_quota_at = None;
        let mut generation = u64::MAX;
        let mut pending = None;
        while !self.stop.load(Ordering::Relaxed) {
            let mut dirty = false;
            let (preferences, next_generation) = self.source_configuration();
            if generation != next_generation {
                generation = next_generation;
                local_quota_at = None;
                dirty = true;
            }
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
                state
                    .quota_owner
                    .as_ref()
                    .map(|owner| owner.source)
                    .unwrap_or(QuotaSource::None)
            };
            if source != next_source {
                source = next_source;
                dirty = true;
            }
            let mut changes = drain(&receive, &lost);
            changes.reconcile |= self.local_refresh.swap(false, Ordering::AcqRel);
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
            if tracked_refresh.elapsed() >= TRACKED_REFRESH {
                dirty |= local.refresh_tracked(&roots, &|| self.stop.load(Ordering::Relaxed));
                tracked_refresh = Instant::now();
            }
            if dirty {
                let sessions = local.snapshot();
                let local_error = local.error.clone();
                let (quotas, observed_at) = local.quotas(roots.first().map(String::as_str));
                let refresh_rpc = local_quota_advanced(source, observed_at, &mut local_quota_at);
                self.publish_local(generation, sessions, local_error, quotas, observed_at);
                if refresh_rpc {
                    self.request_quota_refresh();
                }
            }
            pending = match receive.recv_timeout(WATCH_WAIT) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(WATCH_WAIT);
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
            let (settings, generation) = self.source_configuration();
            let signature = (
                generation,
                settings.executable.clone(),
                settings.roots.first().cloned(),
            );
            if configuration.as_ref() != Some(&signature) {
                if let Some(rpc) = client.take() {
                    rpc::Rpc::close(rpc).await;
                }
                configuration = Some(signature);
                failures = 0;
                auth_paused = false;
            }
            if auth_paused {
                tokio::select! {
                    _ = self.shutdown.notified() => break,
                    _ = self.refresh.notified() => auth_paused = false,
                    _ = self.quota_refresh.notified() => auth_paused = false,
                }
                continue;
            }
            self.publish_generation(generation, |state| {
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
                let source = quota_source(mode);
                let provider = (mode == "externalProvider")
                    .then(|| config::provider(settings.roots.first().map(String::as_str)))
                    .flatten();
                let now = chrono::Utc::now().timestamp_millis();
                let Some(owner) = self.publish_account(
                    generation,
                    mode,
                    account_identity(&account),
                    provider,
                    now,
                ) else {
                    return Ok::<(), String>(());
                };
                if source != QuotaSource::Rpc {
                    if mode == "signedOut" {
                        auth_paused = true;
                    }
                    return Ok::<(), String>(());
                }
                let limits = rpc.call("account/rateLimits/read").await?;
                let now = chrono::Utc::now().timestamp_millis();
                self.publish_rpc(&owner, rpc::quotas(&limits, now), now);
                Ok(())
            };
            let result = tokio::select! {
                result = operation => result,
                _ = self.shutdown.notified() => break,
                _ = self.refresh.notified() => {
                    if let Some(rpc) = client.take() {
                        rpc.close().await;
                    }
                    continue;
                }
            };
            if let Err(error) = result {
                failures = failures.saturating_add(1);
                if error == "auth-required" {
                    auth_paused = true;
                }
                self.publish_generation(generation, |state| {
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
            self.publish_generation(generation, |state| {
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
            let wake = tokio::select! {
                _ = self.shutdown.notified() => break,
                _ = self.refresh.notified() => QueryWake::Manual,
                _ = self.quota_refresh.notified() => QueryWake::Automatic,
                _ = tokio::time::sleep(Duration::from_secs(interval)) => QueryWake::Scheduled,
            };
            let throttle = quota_refresh_throttle(wake, last_query.elapsed());
            if !throttle.is_zero() {
                tokio::select! {
                    _ = self.shutdown.notified() => break,
                    _ = self.refresh.notified() => {},
                    _ = tokio::time::sleep(throttle) => {}
                }
            }
            self.publish_generation(generation, |state| {
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
