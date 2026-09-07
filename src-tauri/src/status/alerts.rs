use super::{Activity, Quality, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub title: &'static str,
    pub body: String,
    pub kind: AlertKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Status,
    Approval,
    Completion,
    Quota,
}

#[derive(Default)]
pub struct Alerts {
    initialized: bool,
    sessions: BTreeMap<String, (Option<i64>, Option<Activity>)>,
    quotas: BTreeMap<String, bool>,
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
                let alert = match session.activity.value {
                    Some(Activity::WaitingApproval) => {
                        Some(("Approval needed", AlertKind::Approval))
                    }
                    Some(Activity::WaitingInput) => Some(("Input needed", AlertKind::Status)),
                    Some(Activity::Completed) => Some(("Task completed", AlertKind::Completion)),
                    Some(Activity::Failed) => Some(("Task failed", AlertKind::Status)),
                    _ => None,
                };
                if let Some((title, kind)) = alert {
                    alerts.push(Alert {
                        title,
                        body: if session.project.is_empty() {
                            "Session".into()
                        } else {
                            session.project.clone()
                        },
                        kind,
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
                let previous = self.quotas.insert(key, low);
                if low && previous.is_none_or(|old| !old) {
                    alerts.push(Alert {
                        title: "Quota low",
                        body: format!(
                            "{}: {:.0}% remaining",
                            quota.name,
                            window.remaining.value.unwrap_or_default()
                        ),
                        kind: AlertKind::Quota,
                    });
                }
            }
        }
        self.quotas.retain(|id, _| quota_keys.contains(id));
        if alerts.iter().any(|alert| alert.kind == AlertKind::Quota) {
            alerts.retain(|alert| alert.kind != AlertKind::Completion);
        }
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
        let alert = &tracker.observe(&snapshot, 10)[0];
        assert_eq!(alert.title, "Task completed");
        assert_eq!(alert.kind, AlertKind::Completion);
        assert!(tracker.observe(&snapshot, 10).is_empty());
    }

    #[test]
    fn waiting_for_command_approval_emits_an_approval_alert_once() {
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

        snapshot.sessions[0].apply(Event::Waiting {
            at: 2,
            turn: Some("turn".into()),
            activity: Activity::WaitingApproval,
        });
        let alerts = tracker.observe(&snapshot, 10);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].title, "Approval needed");
        assert_eq!(alerts[0].kind, AlertKind::Approval);
        assert!(tracker.observe(&snapshot, 10).is_empty());
    }

    #[test]
    fn low_quota_notifies_once_per_threshold_crossing() {
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
        assert!(tracker.observe(&snapshot, 10).is_empty());
    }

    #[test]
    fn task_completion_does_not_repeat_an_active_low_quota_alert() {
        let mut session = session();
        session.apply(Event::TurnStarted {
            at: 1,
            turn: "turn".into(),
        });
        let mut remaining = Field::absent("local", Quality::Unavailable);
        remaining.set(9.0, 1);
        let mut snapshot = Snapshot {
            sessions: vec![session],
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
        assert_eq!(tracker.observe(&snapshot, 10)[0].kind, AlertKind::Quota);

        snapshot.sessions[0].apply(Event::TurnEnded {
            at: 2,
            turn: "turn".into(),
            activity: Activity::Completed,
            duration_ms: None,
        });
        snapshot.quotas[0].windows[0].resets_at = Some(200);

        let alerts = tracker.observe(&snapshot, 10);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, AlertKind::Completion);
    }

    #[test]
    fn new_low_quota_alert_takes_priority_over_simultaneous_task_completion() {
        let mut session = session();
        session.apply(Event::TurnStarted {
            at: 1,
            turn: "turn".into(),
        });
        let mut remaining = Field::absent("local", Quality::Unavailable);
        remaining.set(20.0, 1);
        let mut snapshot = Snapshot {
            sessions: vec![session],
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
        assert!(tracker.observe(&snapshot, 10).is_empty());

        snapshot.sessions[0].apply(Event::TurnEnded {
            at: 2,
            turn: "turn".into(),
            activity: Activity::Completed,
            duration_ms: None,
        });
        snapshot.quotas[0].windows[0].remaining.set(9.0, 2);

        let alerts = tracker.observe(&snapshot, 10);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, AlertKind::Quota);
    }
}
