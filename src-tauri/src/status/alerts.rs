use super::{Activity, Quality, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub title: &'static str,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct QuotaState {
    reset: Option<i64>,
    low: bool,
}

#[derive(Default)]
pub struct Alerts {
    initialized: bool,
    sessions: BTreeMap<String, (Option<i64>, Option<Activity>)>,
    quotas: BTreeMap<String, QuotaState>,
}

impl Alerts {
    pub fn observe(&mut self, snapshot: &Snapshot, threshold: u8) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let mut session_keys = BTreeSet::new();
        for session in &snapshot.sessions {
            session_keys.insert(session.id.clone());
            let current = (session.activity.observed_at, session.activity.value);
            let previous = self.sessions.insert(session.id.clone(), current);
            if self.initialized && previous.is_some() && previous != Some(current) {
                let title = match session.activity.value {
                    Some(Activity::WaitingApproval) => Some("Approval needed"),
                    Some(Activity::WaitingInput) => Some("Input needed"),
                    Some(Activity::Completed) => Some("Task completed"),
                    Some(Activity::Failed) => Some("Task failed"),
                    _ => None,
                };
                if let Some(title) = title {
                    alerts.push(Alert {
                        title,
                        body: if session.project.is_empty() {
                            "Session".into()
                        } else {
                            session.project.clone()
                        },
                    });
                }
            }
        }
        self.sessions.retain(|id, _| session_keys.contains(id));

        let mut quota_keys = BTreeSet::new();
        for quota in &snapshot.quotas {
            for (index, window) in quota.windows.iter().enumerate() {
                let key = format!("{}:{index}", quota.id);
                quota_keys.insert(key.clone());
                let low = window.remaining.quality == Quality::Fresh
                    && window
                        .remaining
                        .value
                        .is_some_and(|value| value <= f64::from(threshold));
                let current = QuotaState {
                    reset: window.resets_at,
                    low,
                };
                let previous = self.quotas.insert(key, current);
                if low && previous.is_none_or(|old| !old.low || old.reset != current.reset) {
                    alerts.push(Alert {
                        title: "Quota low",
                        body: format!(
                            "{}: {:.0}% remaining",
                            quota.name,
                            window.remaining.value.unwrap_or_default()
                        ),
                    });
                }
            }
        }
        self.quotas.retain(|id, _| quota_keys.contains(id));
        self.initialized = true;
        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{Event, Field, Quota, QuotaWindow, Session};

    fn session() -> Session {
        Session::new(
            "root:session".into(),
            "root".into(),
            "/work/project".into(),
            "project".into(),
        )
    }

    #[test]
    fn historical_states_are_seeded_and_transitions_are_deduplicated() {
        let mut session = session();
        session.apply(Event::TurnStarted {
            at: 1,
            turn: "turn".into(),
        });
        let mut snapshot = Snapshot {
            sessions: vec![session],
            ..Snapshot::default()
        };
        let mut tracker = Alerts::default();
        assert!(tracker.observe(&snapshot, 10).is_empty());
        snapshot.sessions[0].apply(Event::TurnEnded {
            at: 2,
            turn: "turn".into(),
            activity: Activity::Completed,
            duration_ms: None,
        });
        assert_eq!(tracker.observe(&snapshot, 10)[0].title, "Task completed");
        assert!(tracker.observe(&snapshot, 10).is_empty());
    }

    #[test]
    fn low_quota_notifies_once_per_threshold_crossing_or_reset() {
        let mut remaining = Field::absent("appServer", Quality::Unavailable);
        remaining.set(9.0, 1);
        let mut snapshot = Snapshot {
            quotas: vec![Quota {
                id: "quota".into(),
                name: "Account".into(),
                windows: vec![QuotaWindow {
                    remaining,
                    minutes: Some(300),
                    resets_at: Some(100),
                }],
                credit_balance: None,
                unlimited_credits: None,
            }],
            ..Snapshot::default()
        };
        let mut tracker = Alerts::default();
        assert_eq!(tracker.observe(&snapshot, 10)[0].title, "Quota low");
        assert!(tracker.observe(&snapshot, 10).is_empty());
        snapshot.quotas[0].windows[0].remaining.value = Some(20.0);
        assert!(tracker.observe(&snapshot, 10).is_empty());
        snapshot.quotas[0].windows[0].remaining.value = Some(8.0);
        assert_eq!(tracker.observe(&snapshot, 10)[0].title, "Quota low");
        assert!(tracker.observe(&snapshot, 10).is_empty());
        snapshot.quotas[0].windows[0].resets_at = Some(200);
        assert_eq!(tracker.observe(&snapshot, 10)[0].title, "Quota low");
    }
}
