use super::{executable, text};
use crate::status::{Field, Quality, Quota, QuotaWindow};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    time::timeout,
};

pub struct Rpc {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    sequence: u64,
}

pub fn quotas(value: &Value, at: i64) -> Vec<Quota> {
    let buckets: Vec<(String, &Value)> = match value["rateLimitsByLimitId"]
        .as_object()
        .filter(|map| !map.is_empty())
    {
        Some(map) => map.iter().map(|(id, value)| (id.clone(), value)).collect(),
        None => vec![(
            value["rateLimits"]["limitId"]
                .as_str()
                .unwrap_or("default")
                .to_owned(),
            &value["rateLimits"],
        )],
    };
    buckets
        .into_iter()
        .filter(|(_, bucket)| bucket.is_object())
        .map(|(id, bucket)| {
            let windows = ["primary", "secondary"]
                .into_iter()
                .filter_map(|key| {
                    let window = &bucket[key];
                    if !window.is_object() {
                        return None;
                    }
                    let mut remaining = Field::absent("appServer", Quality::Unavailable);
                    if let Some(used) = window["usedPercent"]
                        .as_f64()
                        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
                    {
                        remaining.set(100.0 - used, at);
                    }
                    let resets_at = window["resetsAt"]
                        .as_i64()
                        .and_then(|value| value.checked_mul(1000));
                    if resets_at.is_some_and(|reset| reset <= at) && remaining.value.is_some() {
                        remaining.quality = Quality::Stale;
                    }
                    Some(QuotaWindow {
                        remaining,
                        minutes: window["windowDurationMins"]
                            .as_u64()
                            .filter(|value| *value > 0),
                        resets_at,
                    })
                })
                .collect();
            Quota {
                name: text(&bucket["limitName"], 128).unwrap_or_else(|| id.clone()),
                id,
                windows,
                credit_balance: text(&bucket["credits"]["balance"], 64),
                unlimited_credits: bucket["credits"]["unlimited"].as_bool(),
            }
        })
        .collect()
}

impl Rpc {
    pub async fn start(executable: &str, root: Option<&str>) -> Result<Self, String> {
        let mut command = executable::command(executable);
        command
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(root) = root {
            command.env("CODEX_HOME", root);
        }
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command.spawn().map_err(|_| "source-start-failed")?;
        let input = child.stdin.take().ok_or("source-start-failed")?;
        let output = BufReader::new(child.stdout.take().ok_or("source-start-failed")?);
        let mut rpc = Self {
            child,
            input,
            output,
            sequence: 0,
        };
        rpc.call("initialize").await?;
        rpc.input
            .write_all(b"{\"method\":\"initialized\",\"params\":{}}\n")
            .await
            .map_err(|_| "source-write-failed")?;
        Ok(rpc)
    }

    pub async fn call(&mut self, method: &str) -> Result<Value, String> {
        let params = match method {
            "initialize" => json!({"clientInfo":{"name":"codex_status","version":"0.1.0"}}),
            "account/read" => json!({"refreshToken":false}),
            "account/rateLimits/read" => json!({}),
            _ => return Err("forbidden-method".into()),
        };
        self.sequence += 1;
        let id = self.sequence;
        let request = serde_json::to_vec(&json!({"method":method,"params":params,"id":id}))
            .map_err(|_| "request-invalid")?;
        let operation = async {
            self.input
                .write_all(&request)
                .await
                .map_err(|_| "source-write-failed")?;
            self.input
                .write_all(b"\n")
                .await
                .map_err(|_| "source-write-failed")?;
            loop {
                let mut line = Vec::new();
                let count = (&mut self.output)
                    .take(4 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut line)
                    .await
                    .map_err(|_| "source-read-failed")?;
                if count == 0 {
                    return Err("source-exited".into());
                }
                if count > 4 * 1024 * 1024 {
                    return Err("response-too-large".into());
                }
                let message: Value =
                    serde_json::from_slice(&line).map_err(|_| "response-invalid")?;
                if message.get("method").is_some() || message["id"].as_u64() != Some(id) {
                    continue;
                }
                if let Some(error) = message.get("error") {
                    return Err(match error["code"].as_i64() {
                        Some(401 | 403) => "auth-required",
                        Some(-32601) => "unsupported-method",
                        _ => "query-failed",
                    }
                    .into());
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| "response-invalid".into());
            }
        };
        timeout(Duration::from_secs(15), operation)
            .await
            .map_err(|_| "query-timeout".to_owned())?
    }

    pub async fn close(mut self) {
        let _ = self.child.start_kill();
        let _ = timeout(Duration::from_secs(1), self.child.wait()).await;
    }
}

pub async fn version(executable: &str) -> Option<String> {
    let mut command = executable::command(executable);
    command
        .arg("--version")
        .kill_on_drop(true)
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let output = timeout(Duration::from_secs(5), command.output())
        .await
        .ok()?
        .ok()?;
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .find(|v| v.split('.').count() == 3 && v.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quota_zero_missing_invalid_and_expired_remain_distinct() {
        let value = json!({"rateLimitsByLimitId":{"a":{"primary":{"usedPercent":100,"windowDurationMins":45,"resetsAt":100}},"b":{"primary":{"usedPercent":101},"secondary":{}}}});
        let result = quotas(&value, 100_000);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].windows[0].remaining.value, Some(0.0));
        assert_eq!(result[0].windows[0].remaining.quality, Quality::Stale);
        assert_eq!(result[1].windows[0].remaining.value, None);
        assert_eq!(result[1].windows[1].remaining.value, None);
        assert!(quotas(&Value::Null, 0).is_empty());
    }
}
