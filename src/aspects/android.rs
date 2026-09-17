use crate::config::{AspectConfig, QualityAspect};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::path_policy::is_skipped_directory;

/// Auto-detects Gradle tasks for repositories with a Gradle wrapper.
pub fn detect_android_aspects(root: &Path) -> HashMap<QualityAspect, AspectConfig> {
    let Ok(root) = root.canonicalize() else {
        return HashMap::new();
    };
    if wrapper_name(&root).is_none() {
        return HashMap::new();
    }

    let build_content = collect_gradle_build_content(&root);
    if build_content.is_empty() {
        return HashMap::new();
    }

    let mut aspects = HashMap::new();
    if has_android_plugin(&build_content) {
        for (aspect, task) in [
            (QualityAspect::Build, "assembleDebug"),
            (QualityAspect::UnitTest, "testDebugUnitTest"),
            (QualityAspect::Lint, "lintDebug"),
        ] {
            aspects.insert(aspect, gradle_config(task));
        }
        if has_kotlin(&build_content, &root) {
            aspects.insert(
                QualityAspect::TypeCheck,
                gradle_config("compileDebugKotlin"),
            );
        }
    }

    let style_task = if has_plugin_declaration(&build_content, "com.diffplug.spotless") {
        Some("spotlessCheck")
    } else if has_plugin_declaration(&build_content, "org.jlleitschuh.gradle.ktlint") {
        Some("ktlintCheck")
    } else {
        None
    };
    if let Some(task) = style_task {
        aspects.insert(QualityAspect::Style, gradle_config(task));
    }

    aspects
}

fn collect_gradle_build_content(root: &Path) -> String {
    fn visit(path: &Path, root: &Path, content: &mut String) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        let mut entries = entries.flatten().collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() || has_symlink_below_root(&path, root) {
                continue;
            }
            if metadata.is_dir() {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(is_skipped_directory)
                {
                    continue;
                }
                visit(&path, root, content);
            } else if metadata.is_file()
                && matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("build.gradle" | "build.gradle.kts")
                )
                && let Ok(file_content) = fs::read_to_string(path)
            {
                content.push_str(&file_content);
                content.push('\n');
            }
        }
    }

    let mut content = String::new();
    visit(root, root, &mut content);
    content
}

fn has_kotlin(build_content: &str, root: &Path) -> bool {
    [
        "org.jetbrains.kotlin",
        "org.jetbrains.kotlin.android",
        "org.jetbrains.kotlin.jvm",
        "kotlin-android",
        "org.jetbrains.kotlin.multiplatform",
        "kotlin(\"android\")",
        "kotlin(\"jvm\")",
    ]
    .iter()
    .any(|marker| has_plugin_declaration(build_content, marker))
        || has_kotlin_source(root)
}

fn has_kotlin_source(root: &Path) -> bool {
    fn visit(path: &Path, root: &Path) -> bool {
        let Ok(entries) = fs::read_dir(path) else {
            return false;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() || has_symlink_below_root(&path, root) {
                continue;
            }
            if metadata.is_dir() {
                let skipped = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(is_skipped_directory);
                if !skipped && visit(&path, root) {
                    return true;
                }
            } else if metadata.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("kt"))
            {
                return true;
            }
        }
        false
    }

    visit(root, root)
}

fn has_android_plugin(build_content: &str) -> bool {
    [
        "com.android.application",
        "com.android.library",
        "com.android.test",
        "com.android.dynamic-feature",
        "com.android.base",
    ]
    .iter()
    .any(|marker| has_plugin_declaration(build_content, marker))
}

fn has_plugin_declaration(content: &str, marker: &str) -> bool {
    let tokens = lex_gradle(content);
    let mut plugin_block_depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token == &GradleToken::Word("plugins".to_string())
            && tokens.get(index + 1) == Some(&GradleToken::LBrace)
        {
            plugin_block_depth += 1;
            continue;
        }
        if token == &GradleToken::LBrace {
            if plugin_block_depth > 0
                && !matches!(tokens.get(index.wrapping_sub(1)), Some(GradleToken::Word(word)) if word == "plugins")
            {
                plugin_block_depth += 1;
            }
            continue;
        }
        if token == &GradleToken::RBrace {
            plugin_block_depth = plugin_block_depth.saturating_sub(1);
            continue;
        }
        if plugin_block_depth > 0 && plugin_declaration_matches(&tokens, index, marker) {
            return true;
        }
        if plugin_block_depth == 0 && apply_plugin_matches(&tokens, index, marker) {
            return true;
        }
    }
    false
}

#[derive(Debug, PartialEq, Eq)]
enum GradleToken {
    Word(String),
    String(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    Colon,
}

fn lex_gradle(content: &str) -> Vec<GradleToken> {
    let mut tokens = Vec::new();
    let mut chars = content.chars().peekable();
    let mut block_comment = false;
    while let Some(character) = chars.next() {
        if block_comment {
            if character == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_comment = false;
            }
            continue;
        }
        if character == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == '\n' {
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    block_comment = true;
                    continue;
                }
                _ => {}
            }
        }
        if character == '\'' || character == '"' {
            let triple = chars.peek() == Some(&character) && {
                let mut lookahead = chars.clone();
                lookahead.next();
                lookahead.peek() == Some(&character)
            };
            if triple {
                chars.next();
                chars.next();
            }
            let mut value = String::new();
            let mut escaped = false;
            while let Some(next) = chars.next() {
                if triple {
                    if next == character && chars.peek() == Some(&character) {
                        let mut lookahead = chars.clone();
                        lookahead.next();
                        if lookahead.peek() == Some(&character) {
                            chars.next();
                            chars.next();
                            break;
                        }
                    }
                } else if next == character && !escaped {
                    break;
                }
                value.push(next);
                escaped = next == '\\' && !escaped;
                if next != '\\' {
                    escaped = false;
                }
            }
            tokens.push(GradleToken::String(value));
            continue;
        }
        let token = match character {
            '{' => Some(GradleToken::LBrace),
            '}' => Some(GradleToken::RBrace),
            '(' => Some(GradleToken::LParen),
            ')' => Some(GradleToken::RParen),
            ':' => Some(GradleToken::Colon),
            _ if character.is_ascii_alphanumeric() || matches!(character, '_' | '.') => {
                let mut word = character.to_string();
                while chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphanumeric() || matches!(next, '_' | '.'))
                {
                    word.push(chars.next().unwrap());
                }
                Some(GradleToken::Word(word))
            }
            _ => None,
        };
        if let Some(token) = token {
            tokens.push(token);
        }
    }
    tokens
}

fn plugin_declaration_matches(tokens: &[GradleToken], index: usize, marker: &str) -> bool {
    if let Some(GradleToken::Word(name)) = tokens.get(index) {
        if name == "id" {
            return matches!(tokens.get(index + 1..),
                Some([GradleToken::String(value), ..]) if value == marker)
                || matches!(tokens.get(index + 1..),
                    Some([GradleToken::LParen, GradleToken::String(value), GradleToken::RParen, ..]) if value == marker);
        }
        if name == "kotlin"
            && let Some(expected) = marker
                .strip_prefix("kotlin(\"")
                .and_then(|value| value.strip_suffix("\")"))
        {
            return matches!(tokens.get(index + 1..),
                Some([GradleToken::LParen, GradleToken::String(value), GradleToken::RParen, ..]) if value == expected);
        }
    }
    false
}

fn apply_plugin_matches(tokens: &[GradleToken], index: usize, marker: &str) -> bool {
    matches!(tokens.get(index..), Some([
        GradleToken::Word(apply), GradleToken::Word(plugin), GradleToken::Colon,
        GradleToken::String(value), ..
    ]) if apply == "apply" && plugin == "plugin" && value == marker)
}

fn wrapper_name(root: &Path) -> Option<&'static str> {
    let name = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    };
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || has_symlink_below_root(&path, root) {
        return None;
    }
    path.canonicalize()
        .ok()
        .filter(|path| path.starts_with(root))
        .map(|_| name)
}

fn has_symlink_below_root(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return true;
        }
    }
    false
}

fn gradle_config(task: &str) -> AspectConfig {
    let wrapper = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "./gradlew"
    };
    AspectConfig {
        command: Some(format!("{wrapper} {task}")),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "qgate_android_test_{}_{}",
                name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_detect_android_aspects_with_gradlew_and_build_gradle_kts() {
        let dir = TestDir::new("kts");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle.kts"),
            "plugins {\n                id(\"com.android.application\")\n                id(\"com.diffplug.spotless\")\n            }", 
        )
        .unwrap();
        fs::write(dir.path().join("App.kt"), "class App").unwrap();

        let detected = detect_android_aspects(dir.path());

        assert_eq!(
            detected
                .get(&QualityAspect::Build)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew assembleDebug")
        );
        assert_eq!(
            detected
                .get(&QualityAspect::UnitTest)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew testDebugUnitTest")
        );
        assert_eq!(
            detected
                .get(&QualityAspect::Lint)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew lintDebug")
        );
        assert_eq!(
            detected
                .get(&QualityAspect::TypeCheck)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew compileDebugKotlin")
        );
        assert_eq!(
            detected
                .get(&QualityAspect::Style)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew spotlessCheck")
        );
    }

    #[test]
    fn test_detect_android_aspects_with_ktlint() {
        let dir = TestDir::new("ktlint");
        File::create(dir.path().join("./gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle"),
            "plugins {\n                id 'com.android.library'\n                id 'org.jlleitschuh.gradle.ktlint'\n            }", 
        )
        .unwrap();

        let detected = detect_android_aspects(dir.path());

        assert_eq!(
            detected
                .get(&QualityAspect::Style)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew ktlintCheck")
        );
        assert!(!detected.contains_key(&QualityAspect::TypeCheck));
    }

    #[test]
    fn test_java_only_android_project_has_no_kotlin_type_check() {
        let dir = TestDir::new("java_only");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle"),
            "plugins { id 'com.android.application' }",
        )
        .unwrap();
        fs::write(
            dir.path().join("MainActivity.java"),
            "class MainActivity {}",
        )
        .unwrap();

        let detected = detect_android_aspects(dir.path());

        assert!(!detected.contains_key(&QualityAspect::TypeCheck));
    }

    #[test]
    fn test_detect_android_aspects_scans_nested_build_scripts() {
        let dir = TestDir::new("nested");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::create_dir(dir.path().join("app")).unwrap();
        fs::write(
            dir.path().join("app/build.gradle.kts"),
            "plugins { id(\"com.android.library\") }",
        )
        .unwrap();

        let detected = detect_android_aspects(dir.path());

        assert_eq!(
            detected
                .get(&QualityAspect::Build)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew assembleDebug")
        );
    }

    #[test]
    fn test_style_plugin_is_detected_without_android_plugin() {
        let dir = TestDir::new("style_only");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle"),
            "apply plugin: 'org.jlleitschuh.gradle.ktlint'",
        )
        .unwrap();

        let detected = detect_android_aspects(dir.path());

        assert!(!detected.contains_key(&QualityAspect::Build));
        assert_eq!(
            detected
                .get(&QualityAspect::Style)
                .and_then(|c| c.command.as_deref()),
            Some("./gradlew ktlintCheck")
        );
    }

    #[test]
    fn comments_and_fake_plugin_ids_are_ignored() {
        let dir = TestDir::new("false_positive");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle"),
            "// apply plugin: 'com.android.application'\n/* id 'com.android.library' */\npluginsFake { id 'com.android.application.fake' }",
        )
        .unwrap();

        assert!(detect_android_aspects(dir.path()).is_empty());
    }

    #[test]
    fn strings_and_comments_cannot_fake_plugin_declarations() {
        let dir = TestDir::new("string_false_positive");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle"),
            r#"def fake = "id('com.android.application')"
println("plugins { id('com.android.application') }")
def triple = """apply plugin: 'com.android.application'"""
// apply plugin: 'com.android.application'
/* plugins { id 'com.android.application' } */"#,
        )
        .unwrap();

        assert!(detect_android_aspects(dir.path()).is_empty());
    }

    #[test]
    fn valid_plugin_declarations_are_detected() {
        let dir = TestDir::new("valid_plugins");
        File::create(dir.path().join("gradlew")).unwrap();
        fs::write(
            dir.path().join("build.gradle"),
            "plugins { id 'com.android.application' }\napply plugin: 'org.jlleitschuh.gradle.ktlint'",
        )
        .unwrap();

        let detected = detect_android_aspects(dir.path());
        assert!(detected.contains_key(&QualityAspect::Build));
        assert!(detected.contains_key(&QualityAspect::Style));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_gradle_candidates_are_ignored() {
        use std::os::unix::fs::symlink;

        let dir = TestDir::new("symlinked_candidates");
        File::create(dir.path().join("gradlew")).unwrap();
        let outside = TestDir::new("symlink_target");
        fs::write(
            outside.path().join("build.gradle"),
            "plugins { id 'com.android.application' }",
        )
        .unwrap();
        symlink(
            outside.path().join("build.gradle"),
            dir.path().join("build.gradle"),
        )
        .unwrap();

        assert!(detect_android_aspects(dir.path()).is_empty());
    }

    #[test]
    fn test_detect_android_aspects_non_android_repo() {
        let dir = TestDir::new("non_android");
        let detected = detect_android_aspects(dir.path());
        assert!(detected.is_empty());
    }
}
