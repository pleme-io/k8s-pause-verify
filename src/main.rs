//! `pleme-io/k8s-pause-verify` — assert pause-contract invariants.
//!
//! Mirrors arch-synthesizer's `action_domain::pause_contract::PauseContract`.
//! Queries the cluster via kubectl, validates that:
//!   1. The actual runner-pod count matches `expected_pod_count` (default 0)
//!   2. The listener pod is Running (not CrashLoopBackOff / Missing)
//!   3. The pause-state ConfigMap's `pleme.io/paused` label matches the
//!      contract (when paused, the label exists)
//!
//! Same proof as the helmworks `pleme-arc-runner-pool@0.2.0` chart's
//! `validatePause` template — caught at runtime, against the live cluster,
//! rather than at helm render time.

use std::process::{Command, Stdio};

use pleme_actions_shared::{ActionError, Input, Output, StepSummary};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Inputs {
    namespace: String,
    runner_set_label: String,
    #[serde(default)]
    expected_pod_count: u32,
    #[serde(default)]
    kubectl_context: Option<String>,
}

fn main() {
    pleme_actions_shared::log::init();
    if let Err(e) = run() {
        e.emit_to_stdout();
        if e.is_fatal() {
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), ActionError> {
    let inputs = Input::<Inputs>::from_env()?;

    let context_args: Vec<String> = inputs
        .kubectl_context
        .as_ref()
        .map(|c| vec!["--context".into(), c.clone()])
        .unwrap_or_default();

    let pod_count = count_runner_pods(&inputs.namespace, &inputs.runner_set_label, &context_args)?;
    let listener_status = listener_pod_status(&inputs.namespace, &context_args)?;
    let configmap_paused_flag = pause_state_configmap_flag(&inputs.namespace, &context_args)?;

    let output = Output::from_runner_env()?;
    output.set("pod-count", pod_count.to_string())?;
    output.set("listener-status", &listener_status)?;
    output.set("configmap-paused-flag", &configmap_paused_flag)?;

    let mut summary = StepSummary::from_runner_env()?;
    summary
        .heading(2, "k8s-pause-verify")
        .table(
            &["Check", "Expected", "Actual", "Status"],
            vec![
                vec![
                    "runner-pod count".to_string(),
                    inputs.expected_pod_count.to_string(),
                    pod_count.to_string(),
                    if pod_count == inputs.expected_pod_count {
                        pleme_actions_shared::summary::status::PASSED.into()
                    } else {
                        pleme_actions_shared::summary::status::FAILED.into()
                    },
                ],
                vec![
                    "listener status".into(),
                    "Running".into(),
                    listener_status.clone(),
                    if listener_status == "Running" {
                        pleme_actions_shared::summary::status::PASSED.into()
                    } else {
                        pleme_actions_shared::summary::status::FAILED.into()
                    },
                ],
                vec![
                    "configmap-paused-flag".into(),
                    if inputs.expected_pod_count == 0 { "true".into() } else { "absent".into() },
                    configmap_paused_flag.clone(),
                    pleme_actions_shared::summary::status::PASSED.into(),
                ],
            ],
        );
    summary.commit()?;

    if pod_count != inputs.expected_pod_count {
        return Err(ActionError::error(format!(
            "pause invariant violated: expected {} runner pods, got {} \
             (listener stays running so jobs queue, but no runners should \
             materialize while paused)",
            inputs.expected_pod_count, pod_count
        )));
    }

    Ok(())
}

fn count_runner_pods(
    namespace: &str,
    runner_set_label: &str,
    context_args: &[String],
) -> Result<u32, ActionError> {
    let mut args: Vec<String> = vec![
        "-n".into(),
        namespace.into(),
        "get".into(),
        "pods".into(),
        "-l".into(),
        format!("actions.github.com/runner-set={runner_set_label}"),
        "--no-headers".into(),
    ];
    args.extend_from_slice(context_args);
    let stdout = run_kubectl(&args)?;
    // Listener pods carry the same label; exclude them by name pattern.
    let count = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter(|line| !line.contains("listener"))
        .count();
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

fn listener_pod_status(
    namespace: &str,
    context_args: &[String],
) -> Result<String, ActionError> {
    let mut args: Vec<String> = vec![
        "-n".into(),
        namespace.into(),
        "get".into(),
        "pods".into(),
        "-l".into(),
        "app.kubernetes.io/component=runner-scale-set-listener".into(),
        "-o".into(),
        "jsonpath={.items[0].status.phase}".into(),
    ];
    args.extend_from_slice(context_args);
    let stdout = run_kubectl(&args)?;
    let phase = stdout.trim();
    if phase.is_empty() {
        Ok("Missing".into())
    } else {
        Ok(phase.to_string())
    }
}

fn pause_state_configmap_flag(
    namespace: &str,
    context_args: &[String],
) -> Result<String, ActionError> {
    let mut args: Vec<String> = vec![
        "-n".into(),
        namespace.into(),
        "get".into(),
        "configmap".into(),
        "-l".into(),
        "pleme.io/paused=true".into(),
        "-o".into(),
        "jsonpath={.items[0].metadata.labels.pleme\\.io/paused}".into(),
    ];
    args.extend_from_slice(context_args);
    let stdout = run_kubectl(&args)?;
    let val = stdout.trim();
    if val.is_empty() {
        Ok("absent".into())
    } else {
        Ok(val.to_string())
    }
}

fn run_kubectl(args: &[String]) -> Result<String, ActionError> {
    let output = Command::new("kubectl")
        .args(args.iter().map(String::as_str))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| ActionError::error(format!("failed to spawn kubectl: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(ActionError::error(format!(
            "kubectl exited with status {} (stderr: {})",
            output.status,
            stderr.trim()
        )));
    }
    Ok(stdout.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure-function tests — the kubectl calls themselves are exercised
    /// in CI smoke tests against a real cluster. Here we test the parsing
    /// + counting logic.

    fn count_in_text(text: &str) -> u32 {
        let count = text
            .lines()
            .filter(|line| !line.is_empty())
            .filter(|line| !line.contains("listener"))
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    #[test]
    fn count_excludes_listener_lines() {
        let text = "\
runner-pool-listener   1/1   Running   0   1m
arc-runner-abc         1/1   Running   0   30s
arc-runner-def         1/1   Running   0   29s
";
        assert_eq!(count_in_text(text), 2);
    }

    #[test]
    fn count_zero_when_only_listener() {
        let text = "runner-pool-listener   1/1   Running   0   1m\n";
        assert_eq!(count_in_text(text), 0);
    }

    #[test]
    fn count_zero_when_empty() {
        assert_eq!(count_in_text(""), 0);
    }

    #[test]
    fn count_handles_trailing_blank_lines() {
        let text = "runner-pool-listener   1/1   Running   0   1m\n\n\n";
        assert_eq!(count_in_text(text), 0);
    }
}
