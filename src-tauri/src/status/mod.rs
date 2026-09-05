use serde::{Deserialize, Serialize};

pub mod alerts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    Fresh,
    Stale,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field<T> {
    pub value: Option<T>,
    pub source: String,
    pub observed_at: Option<i64>,
    pub quality: Quality,
}

impl<T> Field<T> {
    pub fn absent(source: &str, quality: Quality) -> Self {
        Self {
            value: None,
            source: source.into(),
            observed_at: None,
            quality,
        }
    }

    pub fn set(&mut self, value: T, at: i64) {
        if self.observed_at.is_none_or(|old| at >= old) {
            self.value = Some(value);
            self.observed_at = Some(at);
            self.quality = Quality::Fresh;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Activity {
    Unknown,
    Idle,
    Running,
    WaitingApproval,
    WaitingInput,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub input: Option<u64>,
    pub cached_input: Option<u64>,
    pub output: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Context {
        at: i64,
        model: Option<String>,
        effort: Option<String>,
    },
    Usage {
        at: i64,
        total: Option<Usage>,
        last: Option<Usage>,
        context_limit: Option<u64>,
    },
    TurnStarted {
        at: i64,
        turn: String,
    },
    Waiting {
        at: i64,
        turn: Option<String>,
        activity: Activity,
    },
    TurnEnded {
        at: i64,
        turn: String,
        activity: Activity,
        duration_ms: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub root_id: String,
    pub path: String,
    pub project: String,
    pub version: String,
    pub supported: bool,
    pub activity: Field<Activity>,
    pub model: Field<String>,
    pub effort: Field<String>,
    pub usage: Field<Usage>,
    pub last_usage: Field<Usage>,
    pub context_limit: Field<u64>,
    pub context_used: Field<u64>,
    pub latest_at: i64,
    pub turn_started_at: Option<i64>,
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    pub turn_id: Option<String>,
}

impl Session {
    pub fn new(
        id: String,
        root_id: String,
        path: String,
        project: String,
        version: String,
        supported: bool,
    ) -> Self {
        let quality = if supported {
            Quality::Unavailable
        } else {
            Quality::Unsupported
        };
        Self {
            id,
            root_id,
            path,
            project,
            version,
            supported,
            activity: Field::absent("local", quality),
            model: Field::absent("local", quality),
            effort: Field::absent("local", quality),
            usage: Field::absent("local", quality),
            last_usage: Field::absent("local", quality),
            context_limit: Field::absent("local", quality),
            context_used: Field::absent("local", Quality::Unsupported),
            latest_at: 0,
            turn_started_at: None,
            duration_ms: None,
            turn_id: None,
        }
    }

    pub fn apply(&mut self, event: Event) {
        if !self.supported {
            return;
        }
        let at = match &event {
            Event::Context { at, .. }
            | Event::Usage { at, .. }
            | Event::TurnStarted { at, .. }
            | Event::Waiting { at, .. }
            | Event::TurnEnded { at, .. } => *at,
        };
        self.latest_at = self.latest_at.max(at);
        match event {
            Event::Context { model, effort, .. } => {
                if let Some(model) = model {
                    self.model.set(model, at);
                }
                if let Some(effort) = effort {
                    self.effort.set(effort, at);
                }
            }
            Event::Usage {
                total,
                last,
                context_limit,
                ..
            } => {
                if let Some(value) = total {
                    self.usage.set(value, at);
                }
                if let Some(value) = last {
                    self.last_usage.set(value, at);
                }
                if let Some(limit) = context_limit.filter(|value| *value > 0) {
                    self.context_limit.set(limit, at);
                }
            }
            Event::TurnStarted { turn, .. } => {
                if self.activity.observed_at.is_some_and(|old| at < old)
                    || self.turn_id.as_ref() == Some(&turn)
                        && matches!(
                            self.activity.value,
                            Some(Activity::Completed | Activity::Interrupted | Activity::Failed)
                        )
                {
                    return;
                }
                self.turn_id = Some(turn);
                self.turn_started_at = Some(at);
                self.duration_ms = None;
                self.activity.set(Activity::Running, at);
            }
            Event::Waiting { turn, activity, .. } => {
                if !matches!(activity, Activity::WaitingApproval | Activity::WaitingInput)
                    || self.activity.observed_at.is_some_and(|old| at < old)
                    || turn
                        .as_ref()
                        .is_some_and(|turn| self.turn_id.as_ref().is_some_and(|id| id != turn))
                    || matches!(
                        self.activity.value,
                        Some(Activity::Completed | Activity::Interrupted | Activity::Failed)
                    )
                {
                    return;
                }
                if let Some(turn) = turn {
                    self.turn_id = Some(turn);
                }
                self.activity.set(activity, at);
            }
            Event::TurnEnded {
                turn,
                activity,
                duration_ms,
                ..
            } => {
                if !matches!(
                    activity,
                    Activity::Completed | Activity::Interrupted | Activity::Failed
                ) || self.activity.observed_at.is_some_and(|old| at < old)
                    || self.turn_id.as_ref().is_some_and(|id| id != &turn)
                {
                    return;
                }
                self.turn_id = Some(turn);
                self.duration_ms = duration_ms;
                self.activity.set(activity, at);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub id: String,
    pub name: String,
    pub windows: Vec<QuotaWindow>,
    pub credit_balance: Option<String>,
    pub unlimited_credits: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub remaining: Field<f64>,
    pub minutes: Option<u64>,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub sessions: Vec<Session>,
    pub quotas: Vec<Quota>,
    pub connection: String,
    pub account: String,
    pub provider: Option<String>,
    pub version: Option<String>,
    pub updated_at: Option<i64>,
    pub error: Option<String>,
    pub local_error: Option<String>,
    pub refreshing: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            sessions: vec![],
            quotas: vec![],
            connection: "connecting".into(),
            account: "unknown".into(),
            provider: None,
            version: None,
            updated_at: None,
            error: None,
            local_error: None,
            refreshing: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(root: &str) -> Session {
        Session::new(
            format!("{root}:one"),
            root.into(),
            "/work/project".into(),
            "project".into(),
            "0.153.4".into(),
            true,
        )
    }
    fn start(at: i64, turn: &str) -> Event {
        Event::TurnStarted {
            at,
            turn: turn.into(),
        }
    }
    fn end(at: i64, turn: &str, activity: Activity) -> Event {
        Event::TurnEnded {
            at,
            turn: turn.into(),
            activity,
            duration_ms: None,
        }
    }
    #[test]
    fn old_and_duplicate_events_do_not_reopen_finished_turns() {
        let mut state = session("/first");
        state.apply(start(1, "a"));
        state.apply(end(2, "a", Activity::Completed));
        state.apply(start(1, "a"));
        assert_eq!(state.activity.value, Some(Activity::Completed));
        state.apply(start(3, "b"));
        state.apply(end(4, "a", Activity::Completed));
        assert_eq!(state.activity.value, Some(Activity::Running));
        assert_ne!(state.id, session("/second").id);
    }
    #[test]
    fn cumulative_usage_is_replaced_and_is_not_context_occupancy() {
        let mut state = session("/root");
        let event = Event::Usage {
            at: 1,
            total: Some(Usage {
                input: Some(100),
                cached_input: Some(80),
                output: Some(20),
                total: Some(120),
            }),
            last: None,
            context_limit: Some(1000),
        };
        state.apply(event.clone());
        state.apply(event);
        assert_eq!(state.usage.value.unwrap().total, Some(120));
        assert_eq!(state.context_used.quality, Quality::Unsupported);
    }
    #[test]
    fn waiting_requires_a_current_turn_and_terminal_events_are_explicit() {
        let mut state = session("/root");
        state.apply(start(1, "current"));
        state.apply(Event::Waiting {
            at: 2,
            turn: Some("older".into()),
            activity: Activity::WaitingInput,
        });
        assert_eq!(state.activity.value, Some(Activity::Running));
        state.apply(Event::Waiting {
            at: 3,
            turn: Some("current".into()),
            activity: Activity::WaitingApproval,
        });
        assert_eq!(state.activity.value, Some(Activity::WaitingApproval));
        state.apply(end(4, "current", Activity::Failed));
        assert_eq!(state.activity.value, Some(Activity::Failed));
        state.apply(Event::Waiting {
            at: 5,
            turn: Some("current".into()),
            activity: Activity::WaitingInput,
        });
        assert_eq!(state.activity.value, Some(Activity::Failed));
    }
}
