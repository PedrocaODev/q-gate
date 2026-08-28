use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("qgate_cli_{name}_{}_{suffix}", std::process::id()));
        fs::create_dir_all(&path).expect("create temporary audit directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write_config(&self, aspects: &str) {
        fs::write(
            self.path().join("qgate.toml"),
            format!(
                "[analysis]\ntargets = [\"java\", \"kotlin\"]\nscope = \"diff\"\n\n[rules.god_class]\nmax_methods = 20\nmax_loc = 500\n\n[rules.layers]\n{aspects}"
            ),
        )
        .expect("write qgate config");
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_q-gate"))
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run q-gate")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn advisories_only_audit_prints_advisories_and_passes() {
    let dir = TempDir::new("advisory");
    dir.write_config("\n[aspects.style]\nenabled = true\n");

    let output = dir.run(&["--scope", "full"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "stdout: {stdout}");
    assert!(stdout.contains("Advisories"), "stdout: {stdout}");
}

#[test]
fn failed_configured_aspect_prints_violations_and_fails() {
    let dir = TempDir::new("violation");
    dir.write_config("\n[aspects.lint]\ncommand = \"exit 1\"\nenabled = true\n");

    let output = dir.run(&["--scope", "full"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(output.status.code(), Some(1), "stdout: {stdout}");
    assert!(stdout.contains("Violations"), "stdout: {stdout}");
}

#[test]
fn full_scope_runs_pre_push_checks_that_diff_scope_skips() {
    let dir = TempDir::new("scope");
    dir.write_config(
        "\n[aspects.build]\ncommand = \"echo invoked > scope-marker.txt\"\nstage = \"pre-push\"\nenabled = true\n",
    );
    let git = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("initialize temporary git repository");
    assert!(git.success(), "git init failed");

    let diff = dir.run(&["--scope", "diff"]);
    assert!(diff.status.success());
    assert!(!dir.path().join("scope-marker.txt").exists());

    let full = dir.run(&["--scope", "full"]);
    assert!(full.status.success());
    let marker = dir.path().join("scope-marker.txt");
    assert!(
        marker.is_file(),
        "full scope should invoke the configured check"
    );
    assert_eq!(fs::read_to_string(marker).unwrap(), "invoked\n");
}
