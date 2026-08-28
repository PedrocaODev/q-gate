use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn get_changed_files_from(root: &Path) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "-z"])
        .output()
        .map_err(|e| anyhow!("Failed to execute git status: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let mut records = output.stdout.split(|byte| *byte == 0);
    let mut changed_files = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        let is_rename_or_copy = status.contains(&b'R') || status.contains(&b'C');
        if is_rename_or_copy {
            // -z emits the new path and then the old path as adjacent records.
            let _old_path = records.next();
        }
        if status.contains(&b'D') {
            continue;
        }

        let path = PathBuf::from(String::from_utf8_lossy(&record[3..]).into_owned());
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("java" | "kt")
        ) {
            changed_files.push(root.join(path));
        }
    }

    Ok(changed_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct CurrentDir(PathBuf);

    impl CurrentDir {
        fn enter(path: PathBuf) -> Self {
            let old = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self(old)
        }
    }

    impl Drop for CurrentDir {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
        }
    }

    #[test]
    fn status_uses_new_rename_path_and_skips_deleted_files() {
        let root = std::env::temp_dir().join(format!(
            "qgate_git_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let _cwd = CurrentDir::enter(root.clone());
        let run = |args: &[&str]| Command::new("git").args(args).output().unwrap();
        assert!(run(&["init", "--quiet"]).status.success());
        assert!(
            run(&["config", "user.email", "qgate@example.com"])
                .status
                .success()
        );
        assert!(run(&["config", "user.name", "qgate"]).status.success());
        fs::write("old.java", "class Old {}").unwrap();
        fs::write("deleted.kt", "class Deleted {}").unwrap();
        assert!(run(&["add", "."]).status.success());
        assert!(
            run(&["commit", "--quiet", "-m", "initial"])
                .status
                .success()
        );
        fs::rename("old.java", "new.java").unwrap();
        fs::remove_file("deleted.kt").unwrap();

        let changed = get_changed_files_from(Path::new(".")).unwrap();
        assert_eq!(changed, vec![PathBuf::from("./new.java")]);
        fs::remove_dir_all(root).unwrap();
    }
}
