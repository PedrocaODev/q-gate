use crate::config::{Advisory, AspectConfig, QualityAspect, Severity};
use crate::rules::Violation;
use std::collections::HashMap;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const OUTPUT_TRUNCATION_MARKER: &[u8] = b"\n...[output truncated]";
const TIMEOUT_OUTPUT_MARKER: &str = "command timed out";
const ADB_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
}

#[derive(Debug, Default)]
pub struct Orchestrator;

impl Orchestrator {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_command(&self, cmd: &str, timeout_secs: u64, cwd: &Path) -> CommandOutput {
        let mut command = shell_command(cmd);
        command.current_dir(cwd);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return CommandOutput {
                    stdout: String::new(),
                    stderr: error.to_string(),
                    exit_code: -1,
                    timed_out: false,
                };
            }
        };

        let stdout_output = Arc::new(Mutex::new(Vec::with_capacity(MAX_OUTPUT_BYTES)));
        let stderr_output = Arc::new(Mutex::new(Vec::with_capacity(MAX_OUTPUT_BYTES)));
        let (stdout_tx, stdout_rx) = mpsc::channel();
        let (stderr_tx, stderr_rx) = mpsc::channel();
        if let Some(mut stdout) = child.stdout.take() {
            let output = Arc::clone(&stdout_output);
            thread::spawn(move || read_output(&mut stdout, output, stdout_tx));
        }
        if let Some(mut stderr) = child.stderr.take() {
            let output = Arc::clone(&stderr_output);
            thread::spawn(move || read_output(&mut stderr, output, stderr_tx));
        }

        let timeout = Duration::from_secs(timeout_secs);
        let started = Instant::now();
        let mut timed_out = false;
        let exit_code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) if started.elapsed() >= timeout => {
                    timed_out = true;
                    terminate_child(&mut child);
                    // Reap the shell after terminating its process group. This also
                    // closes the pipes so the bounded reader threads can finish.
                    break child
                        .wait()
                        .ok()
                        .and_then(|status| status.code())
                        .unwrap_or(-1);
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    terminate_child(&mut child);
                    let _ = child.wait();
                    let mut stderr = error.to_string().into_bytes();
                    wait_for_output(&stdout_rx, true);
                    wait_for_output(&stderr_rx, true);
                    stderr.extend(output_bytes(&stderr_output));
                    return CommandOutput {
                        stdout: String::from_utf8_lossy(&output_bytes(&stdout_output)).into_owned(),
                        stderr: String::from_utf8_lossy(&stderr).into_owned(),
                        exit_code: -1,
                        timed_out,
                    };
                }
            }
        };

        // A normal child closes both pipes, so wait for each reader. A killed
        // descendant may keep a pipe open; retain what arrived before the bound.
        wait_for_output(&stdout_rx, timed_out);
        wait_for_output(&stderr_rx, timed_out);
        let stdout = output_bytes(&stdout_output);
        let mut stderr = output_bytes(&stderr_output);
        if timed_out && !stderr.ends_with(TIMEOUT_OUTPUT_MARKER.as_bytes()) {
            if !stderr.is_empty() {
                stderr.push(b'\n');
            }
            stderr.extend_from_slice(TIMEOUT_OUTPUT_MARKER.as_bytes());
        }

        CommandOutput {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            exit_code,
            timed_out,
        }
    }

    pub fn run_aspects(
        &self,
        aspects: &HashMap<QualityAspect, AspectConfig>,
        root: &Path,
    ) -> (Vec<Violation>, Vec<Advisory>) {
        let mut configured = aspects.iter().collect::<Vec<_>>();
        configured.sort_by_key(|(aspect, _)| aspect.to_string());

        let mut violations = Vec::new();
        let mut advisories = Vec::new();

        for (aspect, config) in configured {
            if config.enabled == Some(false) || config.severity == Some(Severity::Ignore) {
                continue;
            }

            let Some(command) = config
                .command
                .as_deref()
                .filter(|command| !command.trim().is_empty())
            else {
                advisories.push(Advisory {
                    aspect: *aspect,
                    message: format!("No command is configured for the {aspect} quality aspect."),
                    recommendation: format!(
                        "Configure a command for {aspect} in qgate.toml or install its tooling."
                    ),
                    fingerprint: format!("aspect:{aspect}:missing-command"),
                });
                continue;
            };

            let output = self.execute_command(
                command,
                config.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS),
                root,
            );
            if output.timed_out || output.exit_code != 0 {
                let details = format_output(&output);
                let severity = config.severity.unwrap_or(Severity::Error);
                let no_device = *aspect == QualityAspect::IntegrationTest
                    && has_connected_android_device() == Some(false)
                    && is_device_unavailable_failure(&details);
                if matches!(severity, Severity::Advisory)
                    || (*aspect == QualityAspect::IntegrationTest
                        && no_device
                        && config.severity.is_none())
                {
                    advisories.push(Advisory {
                        aspect: *aspect,
                        message: format!("Quality aspect {aspect} failed: {details}"),
                        recommendation: "Fix the reported quality check failure.".to_string(),
                        fingerprint: format!("aspect:{aspect}"),
                    });
                } else {
                    violations.push(Violation {
                        file: ".".to_string(),
                        line: 0,
                        rule: format!("aspect:{aspect}"),
                        message: format!("Quality aspect {aspect} failed: {details}"),
                        severity: severity.to_string(),
                        fingerprint: format!("aspect:{aspect}"),
                    });
                }
            }
        }

        (violations, advisories)
    }
}

fn wait_for_output(receiver: &mpsc::Receiver<()>, timed_out: bool) {
    // Descendants can inherit a pipe after the shell exits. Do not let them
    // turn cleanup into an unbounded wait; bytes are captured as they arrive.
    let wait = if timed_out {
        Duration::from_millis(250)
    } else {
        Duration::from_secs(5)
    };
    let _ = receiver.recv_timeout(wait);
}

fn output_bytes(output: &Arc<Mutex<Vec<u8>>>) -> Vec<u8> {
    output
        .lock()
        .expect("output capture mutex poisoned")
        .clone()
}

fn read_output(reader: &mut impl Read, output: Arc<Mutex<Vec<u8>>>, done: mpsc::Sender<()>) {
    let output_limit = MAX_OUTPUT_BYTES.saturating_sub(OUTPUT_TRUNCATION_MARKER.len());
    let mut buffer = [0u8; 8 * 1024];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let mut output = output.lock().expect("output capture mutex poisoned");
                let remaining = output_limit.saturating_sub(output.len());
                let kept = remaining.min(read);
                output.extend_from_slice(&buffer[..kept]);
                truncated |= kept < read;
            }
        }
    }
    if truncated {
        output
            .lock()
            .expect("output capture mutex poisoned")
            .extend_from_slice(OUTPUT_TRUNCATION_MARKER);
    }
    let _ = done.send(());
}

fn has_connected_android_device() -> Option<bool> {
    let mut child = Command::new("adb")
        .arg("devices")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) => {
                let output = child.wait_with_output().ok()?;
                return Some(
                    status.success()
                        && adb_output_has_connected_device(&String::from_utf8_lossy(
                            &output.stdout,
                        )),
                );
            }
            None if started.elapsed() >= ADB_PROBE_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn adb_output_has_connected_device(output: &str) -> bool {
    output.lines().any(|line| {
        line.trim_end_matches('\r')
            .split('\t')
            .nth(1)
            .is_some_and(|state| state == "device")
    })
}

fn is_device_unavailable_failure(details: &str) -> bool {
    let details = details.to_ascii_lowercase();
    [
        "no devices",
        "no connected device",
        "device not found",
        "no such device",
        "unable to locate a device",
        "waiting for device",
        "emulator not found",
    ]
    .iter()
    .any(|marker| details.contains(marker))
}

fn format_output(output: &CommandOutput) -> String {
    let mut details = output.stdout.clone();
    if !output.stderr.is_empty() {
        if !details.is_empty() {
            details.push('\n');
        }
        details.push_str(&output.stderr);
    }
    if output.timed_out && !details.ends_with(TIMEOUT_OUTPUT_MARKER) {
        if !details.is_empty() {
            details.push('\n');
        }
        details.push_str(TIMEOUT_OUTPUT_MARKER);
    } else if details.is_empty() {
        details = format!("command exited with code {}", output.exit_code);
    }
    details.trim().to_string()
}

fn shell_command(cmd: &str) -> Command {
    #[cfg(unix)]
    {
        let mut command = Command::new("sh");
        command.arg("-c").arg(cmd);
        #[cfg(unix)]
        command.process_group(0);
        command
    }

    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", cmd]);
        command
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn terminate_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // ponytail: kill the shell's process group; a per-process timeout would leak descendants.
        let result = unsafe { kill(-(child.id() as i32), 9) };
        if result != 0 {
            let _ = child.kill();
        }
    }

    #[cfg(windows)]
    {
        // `cmd /C` can leave grandchildren running and holding our pipes open.
        // taskkill's tree mode is best effort; child.kill remains the fallback.
        let mut taskkill = Command::new("taskkill");
        taskkill.args(["/PID", &child.id().to_string(), "/T", "/F"]);
        if let Ok(mut process) = taskkill.spawn() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                match process.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => thread::sleep(Duration::from_millis(10)),
                    Err(_) => break,
                }
            }
            let _ = process.kill();
            let _ = process.wait();
        }
        let _ = child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_command_captures_stdout_stderr_and_exit_code() {
        let orchestrator = Orchestrator::new();
        let output = orchestrator.execute_command("echo test_output", 60, Path::new("."));

        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("test_output"));
        assert!(!output.timed_out);
    }

    #[test]
    fn test_execute_command_timeout() {
        let orchestrator = Orchestrator::new();
        let output = orchestrator.execute_command("sleep 2", 1, Path::new("."));

        assert!(output.timed_out);
        assert_ne!(output.exit_code, 0);
    }

    #[test]
    fn test_execute_command_bounds_noisy_output() {
        let orchestrator = Orchestrator::new();
        let output = orchestrator.execute_command(
            "i=0; while [ $i -lt 100000 ]; do printf x; printf y >&2; i=$((i + 1)); done",
            10,
            Path::new("."),
        );

        assert!(!output.timed_out);
        assert!(output.stdout.len() <= MAX_OUTPUT_BYTES);
        assert!(output.stderr.len() <= MAX_OUTPUT_BYTES);
        assert!(output.stdout.contains("output truncated"));
        assert!(output.stderr.contains("output truncated"));
    }

    #[test]
    fn test_run_aspects_failing_command_returns_violation() {
        let orchestrator = Orchestrator::new();
        let mut aspects = HashMap::new();
        aspects.insert(
            QualityAspect::Lint,
            AspectConfig {
                command: Some("sh -c 'echo lint_error >&2; exit 1'".to_string()),
                enabled: Some(true),
                ..Default::default()
            },
        );

        let (violations, advisories) = orchestrator.run_aspects(&aspects, Path::new("."));

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule, "aspect:lint");
        assert!(violations[0].message.contains("lint_error"));
        assert!(advisories.is_empty());
    }

    #[test]
    fn adb_parser_accepts_only_device_state() {
        assert!(adb_output_has_connected_device(
            "List of devices attached\nserial-1\tdevice\n"
        ));
        assert!(!adb_output_has_connected_device(
            "List of devices attached\nserial-1\toffline\nserial-2\tunauthorized\n"
        ));
        assert!(!adb_output_has_connected_device("serial-1 device\n"));
    }

    #[test]
    fn test_run_aspects_missing_command_returns_advisory() {
        let orchestrator = Orchestrator::new();
        let mut aspects = HashMap::new();
        aspects.insert(
            QualityAspect::Style,
            AspectConfig {
                command: None,
                enabled: Some(true),
                ..Default::default()
            },
        );

        let (violations, advisories) = orchestrator.run_aspects(&aspects, Path::new("."));

        assert!(violations.is_empty());
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].aspect, QualityAspect::Style);
    }

    #[test]
    fn aspect_fingerprint_does_not_contain_command_text() {
        let command = "sh -c 'echo secret-command-text >&2; exit 1'";
        let mut aspects = HashMap::new();
        aspects.insert(
            QualityAspect::Lint,
            AspectConfig {
                command: Some(command.to_string()),
                enabled: Some(true),
                ..Default::default()
            },
        );

        let (violations, _) = Orchestrator::new().run_aspects(&aspects, Path::new("."));

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].fingerprint, "aspect:lint");
        assert!(!violations[0].fingerprint.contains(command));
    }
}
