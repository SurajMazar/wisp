//! Tool execution pipeline (spec §55–58, §62, §105):
//! validate → assess risk → policy → (approval) → execute in the host → verify → audit.
//!
//! Nothing the model or UI asks for runs without passing every step. Approvals are answered from
//! the UI (`tool.respond`) and expire after two minutes as a denial.

use std::{collections::HashMap, sync::Mutex, time::Duration};

use tokio::sync::oneshot;
use wisp_ipc::{
    ErrorCode, IpcError,
    methods::{ToolApprovalRequested, ToolApprovalResolved, ToolCallUpdated},
    types::{
        accessibility::{AxSnapshot, AxSnapshotParams},
        app::Empty,
        tools::{
            ApprovalDecision, ApprovalRequest, ApprovalResolved, ApprovalResponse,
            InputPerformParams, ToolCall, ToolCallRecord, ToolCallStatus, ToolExecution,
            ToolGrants, ToolRunParams, ToolRunResult, Verification, VerificationStatus,
        },
    },
};
use wisp_permissions::{Context, Grants, Outcome};

use crate::Core;

pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

struct Pending {
    offered: Vec<ApprovalDecision>,
    tx: oneshot::Sender<ApprovalDecision>,
}

#[derive(Default)]
pub struct ToolService {
    pending: Mutex<HashMap<String, Pending>>,
    /// Tools allowed for the rest of this session (cleared when the engine restarts).
    session: Mutex<Vec<String>>,
}

impl ToolService {
    fn session(&self) -> Vec<String> {
        self.session.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn unavailable(msg: &str) -> IpcError {
    IpcError::new(ErrorCode::Unavailable, msg)
}

impl Core {
    /// For pointer actions, look at what's actually under the point: risk is judged on that
    /// element's label, not on the label the model claimed. Returns (owner pid, app name).
    async fn inspect_target(&self, call: &mut ToolCall) -> (Option<i32>, Option<String>) {
        let (ToolCall::Click(p)
        | ToolCall::DoubleClick(p)
        | ToolCall::RightClick(p)
        | ToolCall::MoveMouse(p)) = call
        else {
            return (None, None);
        };
        let Some(host) = self.ai().host() else {
            return (None, None);
        };
        let params = serde_json::json!({ "point": { "x": p.x, "y": p.y }, "maxNodes": 1 });
        let Ok(v) = host.call("host.ax.snapshot", params).await else {
            return (None, None);
        };
        let Ok(snap) = serde_json::from_value::<AxSnapshot>(v) else {
            return (None, None);
        };
        if let Some(el) = &snap.under_pointer {
            let actual = [&el.title, &el.description, &el.value]
                .into_iter()
                .flatten()
                .map(|t| t.trim())
                .find(|t| !t.is_empty());
            if let Some(actual) = actual {
                p.label = Some(actual.chars().take(200).collect());
            }
        }
        let app = snap.application.as_ref();
        (app.map(|a| a.pid), app.map(|a| a.name.clone()))
    }

    pub(crate) async fn run_tool(&self, p: ToolRunParams) -> Result<ToolRunResult, IpcError> {
        wisp_computer::validate(&p.call).map_err(|e| IpcError::invalid_params(e.to_string()))?;
        let mut p = p;
        let (expected_pid, app_name) = self.inspect_target(&mut p.call).await;
        let tool = wisp_computer::name(&p.call);
        let (risk, risk_reason) = wisp_computer::assess(&p.call);
        let settings = self.blocking(|db| db.load_settings()).await?;
        let grants = self.blocking(|db| db.list_tool_grants()).await?;
        let (always, never): (Vec<_>, Vec<_>) = grants
            .into_iter()
            .partition(|g| g.decision == ApprovalDecision::AlwaysAllow);
        let always: Vec<String> = always.into_iter().map(|g| g.tool).collect();
        let never: Vec<String> = never.into_iter().map(|g| g.tool).collect();
        let session = self.tool_service().session();

        let mut record = ToolCallRecord {
            id: wisp_common::ids::new_id(),
            tool: tool.into(),
            title: wisp_computer::title(&p.call),
            risk,
            decision: String::new(),
            status: ToolCallStatus::WaitingForApproval,
            verification: None,
            error: None,
            message_id: p.message_id.clone(),
            created_at_ms: wisp_common::time::now_ms(),
            finished_at_ms: None,
        };
        let args = wisp_computer::redacted_args(&p.call);
        let outcome = wisp_permissions::evaluate(&Context {
            enabled: settings.computer.enabled,
            always_grantable: wisp_computer::always_grantable(tool),
            confirm_every_action: settings.computer.confirm_every_action,
            tool,
            risk,
            grants: Grants {
                always: &always,
                never: &never,
                session: &session,
            },
        });

        let mut inserted = false;
        match outcome {
            Outcome::Deny(why) => {
                record.decision = "policyDeny".into();
                return Ok(self
                    .finish_tool(record, ToolCallStatus::Denied, None, Some(why), &args, true)
                    .await);
            }
            Outcome::Allow(reason) => record.decision = reason.into(),
            Outcome::Ask(choices) => {
                self.save_tool(&record, &args, true).await;
                inserted = true;
                let decision = self
                    .ask(&record, risk_reason, &p.call, choices, app_name.clone())
                    .await;
                record.decision = match decision {
                    ApprovalDecision::AllowOnce => "allowOnce",
                    ApprovalDecision::AllowSession => "allowSession",
                    ApprovalDecision::AlwaysAllow => "alwaysAllow",
                    ApprovalDecision::Deny => "deny",
                }
                .into();
                match decision {
                    ApprovalDecision::Deny => {
                        return Ok(self
                            .finish_tool(
                                record,
                                ToolCallStatus::Denied,
                                None,
                                Some("Not allowed.".into()),
                                &args,
                                false,
                            )
                            .await);
                    }
                    ApprovalDecision::AllowSession => {
                        if let Ok(mut s) = self.tool_service().session.lock()
                            && !s.iter().any(|t| t == tool)
                        {
                            s.push(tool.into());
                        }
                    }
                    ApprovalDecision::AlwaysAllow => {
                        let t = tool.to_owned();
                        let _ = self
                            .blocking(move |db| {
                                db.set_tool_grant(&t, ApprovalDecision::AlwaysAllow)
                            })
                            .await;
                    }
                    ApprovalDecision::AllowOnce => {}
                }
            }
        }

        record.status = ToolCallStatus::Running;
        self.save_tool(&record, &args, !inserted).await;
        let approval_id = inserted.then(|| record.id.clone());
        match self.execute(&p.call, approval_id, expected_pid).await {
            Ok(exec) => {
                let failed = exec.verification.status == VerificationStatus::Failed;
                let error = failed.then(|| exec.verification.detail.clone());
                let status = if failed {
                    ToolCallStatus::Failed
                } else {
                    ToolCallStatus::Succeeded
                };
                record.verification = Some(exec.verification);
                let mut r = self
                    .finish_tool(record, status, None, error, &args, false)
                    .await;
                r.output = exec.output;
                Ok(r)
            }
            Err(e) => Ok(self
                .finish_tool(
                    record,
                    ToolCallStatus::Failed,
                    None,
                    Some(e.message),
                    &args,
                    false,
                )
                .await),
        }
    }

    /// Ask the user and wait (bounded). Returns Deny on timeout or disconnect.
    async fn ask(
        &self,
        record: &ToolCallRecord,
        risk_reason: Option<String>,
        call: &ToolCall,
        choices: Vec<ApprovalDecision>,
        app_name: Option<String>,
    ) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut p) = self.tool_service().pending.lock() {
            p.insert(
                record.id.clone(),
                Pending {
                    offered: choices.clone(),
                    tx,
                },
            );
        }
        // Show exactly what will be typed or copied; if it's long, say how much isn't shown.
        let text_detail = |t: &str| {
            let n = t.chars().count();
            let shown: String = t.chars().take(2000).collect();
            if n > 2000 {
                format!("{shown}\n… (+{} more characters)", n - 2000)
            } else {
                shown
            }
        };
        self.events()
            .publish::<ToolApprovalRequested>(&ApprovalRequest {
                id: record.id.clone(),
                tool: record.tool.clone(),
                title: record.title.clone(),
                detail: match call {
                    ToolCall::TypeText(t) | ToolCall::WriteClipboard(t) => {
                        Some(text_detail(&t.text))
                    }
                    _ => app_name.map(|a| format!("In {a}")),
                },
                risk: record.risk,
                risk_reason,
                choices,
                expires_at_ms: wisp_common::time::now_ms() + APPROVAL_TIMEOUT.as_millis() as i64,
            });
        let decision = match tokio::time::timeout(APPROVAL_TIMEOUT, rx).await {
            Ok(Ok(d)) => d,
            _ => ApprovalDecision::Deny,
        };
        if let Ok(mut p) = self.tool_service().pending.lock() {
            p.remove(&record.id);
        }
        self.events()
            .publish::<ToolApprovalResolved>(&ApprovalResolved {
                id: record.id.clone(),
                decision,
            });
        decision
    }

    pub(crate) fn respond_tool(&self, r: ApprovalResponse) -> Result<Empty, IpcError> {
        let pending = self
            .tool_service()
            .pending
            .lock()
            .ok()
            .and_then(|mut p| p.remove(&r.id))
            .ok_or_else(|| IpcError::new(ErrorCode::NotFound, "That request already ended."))?;
        let decision = wisp_permissions::accept(&pending.offered, r.decision);
        let _ = pending.tx.send(decision);
        Ok(Empty {})
    }

    async fn execute(
        &self,
        call: &ToolCall,
        approval_id: Option<String>,
        expected_pid: Option<i32>,
    ) -> Result<ToolExecution, IpcError> {
        let host = self
            .ai()
            .host()
            .ok_or_else(|| unavailable("The app isn't connected."))?;
        match call {
            ToolCall::ReadScreen(_) | ToolCall::FindElement(_) => {
                let snap: AxSnapshot = serde_json::from_value(
                    host.call(
                        "host.ax.snapshot",
                        serde_json::to_value(AxSnapshotParams {
                            point: None,
                            region: None,
                            max_nodes: 400,
                        })
                        .unwrap_or_default(),
                    )
                    .await?,
                )
                .map_err(|e| IpcError::internal(e.to_string()))?;
                Ok(read_or_find(call, snap))
            }
            _ => serde_json::from_value(
                host.call(
                    "host.input.perform",
                    serde_json::to_value(InputPerformParams {
                        call: call.clone(),
                        approval_id,
                        expected_pid,
                    })
                    .unwrap_or_default(),
                )
                .await?,
            )
            .map_err(|e| IpcError::internal(e.to_string())),
        }
    }

    async fn save_tool(&self, r: &ToolCallRecord, args: &serde_json::Value, insert: bool) {
        let (r2, a) = (r.clone(), args.clone());
        let res = if insert {
            self.blocking(move |db| db.insert_tool_call(&r2, &a)).await
        } else {
            self.blocking(move |db| db.update_tool_call(&r2)).await
        };
        if let Err(e) = res {
            tracing::warn!(error = %e.message, "couldn't audit tool call");
        }
        self.events().publish::<ToolCallUpdated>(r);
    }

    async fn finish_tool(
        &self,
        mut record: ToolCallRecord,
        status: ToolCallStatus,
        verification: Option<Verification>,
        error: Option<String>,
        args: &serde_json::Value,
        insert: bool,
    ) -> ToolRunResult {
        record.status = status;
        if verification.is_some() {
            record.verification = verification;
        }
        record.error = error;
        record.finished_at_ms = Some(wisp_common::time::now_ms());
        self.save_tool(&record, args, insert).await;
        tracing::info!(tool = %record.tool, risk = record.risk.as_str(), decision = %record.decision, status = ?record.status, "tool call");
        ToolRunResult {
            record,
            output: None,
        }
    }

    pub(crate) async fn tool_grants(&self) -> Result<ToolGrants, IpcError> {
        let grants = self.blocking(|db| db.list_tool_grants()).await?;
        Ok(ToolGrants {
            grants,
            session: self.tool_service().session(),
        })
    }

    /// Revoke a standing grant by id, or a session grant by tool name.
    pub(crate) async fn revoke_tool(&self, id: String) -> Result<ToolGrants, IpcError> {
        let removed_session = self
            .tool_service()
            .session
            .lock()
            .map(|mut s| {
                let before = s.len();
                s.retain(|t| *t != id);
                before != s.len()
            })
            .unwrap_or(false);
        if !removed_session {
            if !wisp_common::ids::is_valid_id(&id) {
                return Err(IpcError::invalid_params("unknown permission"));
            }
            self.blocking(move |db| db.revoke_tool_grant(&id)).await?;
        }
        self.tool_grants().await
    }
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Read-only tools answered from an Accessibility snapshot.
fn read_or_find(call: &ToolCall, snap: AxSnapshot) -> ToolExecution {
    match call {
        ToolCall::FindElement(f) => {
            let want = norm(&f.label);
            let hit = snap
                .nodes
                .iter()
                .chain(snap.under_pointer.iter())
                .find(|n| {
                    n.frame.is_some()
                        && [&n.title, &n.description, &n.value]
                            .into_iter()
                            .flatten()
                            .any(|t| norm(t) == want || norm(t).contains(&want))
                });
            match hit.and_then(|n| n.frame.map(|r| (n, r))) {
                Some((n, r)) => ToolExecution {
                    output: Some(format!(
                        "{} “{}” at x={:.0} y={:.0} ({:.0}×{:.0})",
                        n.role.trim_start_matches("AX"),
                        f.label,
                        r.x + r.width / 2.0,
                        r.y + r.height / 2.0,
                        r.width,
                        r.height
                    )),
                    verification: Verification {
                        status: VerificationStatus::Verified,
                        detail: "Found it on screen".into(),
                    },
                },
                None => ToolExecution {
                    output: None,
                    verification: Verification {
                        status: VerificationStatus::Failed,
                        detail: format!("Couldn't find “{}” in the frontmost window", f.label),
                    },
                },
            }
        }
        _ => {
            let ctx = wisp_accessibility::rank(snap, None, None, 60);
            ToolExecution {
                output: Some(ctx.summary),
                verification: Verification {
                    status: VerificationStatus::Verified,
                    detail: "Read the window's controls".into(),
                },
            }
        }
    }
}
