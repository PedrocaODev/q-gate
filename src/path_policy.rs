use std::path::Path;

pub fn is_skipped_directory(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git" | "target" | "build" | ".gradle" | "generated" | "out" | "node_modules" | "vendor"
    )
}

/// Checks only components below `root`; the root and its ancestors are never excluded.
pub fn has_skipped_component(path: &Path, root: &Path) -> bool {
    let relative = match path.strip_prefix(root) {
        Ok(relative) => Some(relative.to_path_buf()),
        Err(_) => match (root.canonicalize(), path.canonicalize()) {
            (Ok(root), Ok(path)) => path.strip_prefix(&root).ok().map(Path::to_path_buf),
            _ => None,
        },
    };
    relative.is_some_and(|relative| {
        relative.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(is_skipped_directory)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn exclusions_are_relative_to_the_project_root() {
        assert!(!has_skipped_component(
            Path::new("/tmp/build"),
            Path::new("/tmp/build")
        ));
        assert!(!has_skipped_component(
            Path::new("/tmp/build-parent/project/src/Main.java"),
            Path::new("/tmp/build-parent/project"),
        ));
        assert!(has_skipped_component(
            Path::new("/tmp/project/Build/src/Main.java"),
            Path::new("/tmp/project"),
        ));
        assert!(!has_skipped_component(
            Path::new("/tmp/project/src/Main.java"),
            Path::new("/tmp/project"),
        ));
    }
}
