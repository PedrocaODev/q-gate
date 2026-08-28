mod analyzer;
mod aspects;
mod config;
mod git;
mod path_policy;
mod rules;

use analyzer::LanguageAnalyzer;
use analyzer::java::JavaAnalyzer;
use analyzer::kotlin::KotlinAnalyzer;
use anyhow::{Context, Result, anyhow};
use aspects::{Orchestrator, detect_android_aspects};
use clap::Parser;
use config::{AspectConfig, BaselineConfig, Config, QualityAspect, Severity, Stage};
use rules::{
    Rule, Violation, dead_code::DeadCodeRule, duplication::DuplicationRule,
    god_class::GodClassRule, layers::LayerRule,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Analysis scope: "diff" or "full"
    #[arg(short, long)]
    scope: Option<String>,

    /// Specific file or directory to analyze
    #[arg(short, long)]
    path: Option<String>,
}

struct ParsedFile {
    path: String,
    code: String,
    tree: Tree,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let explicit_path = args.path.map(PathBuf::from);
    let discovered_root = find_project_root(explicit_path.as_deref());
    let command_root = canonical_project_root(&discovered_root)?;
    let explicit_path = explicit_path
        .map(|path| resolve_input_path(&command_root, &path))
        .transpose()?;
    let Config {
        analysis,
        rules: rule_config,
        aspects: configured_aspects,
        baseline: baseline_config,
    } = Config::load_from(&command_root.join("qgate.toml"))?;
    let baseline = load_baseline(&command_root, &baseline_config)?;
    let scope = args.scope.unwrap_or(analysis.scope);
    if !matches!(scope.as_str(), "diff" | "full" | "all") {
        return Err(anyhow!(
            "Unsupported analysis scope '{scope}'. Expected 'diff' or 'full' (alias: 'all')."
        ));
    }
    let full_scope = matches!(scope.as_str(), "full" | "all");

    println!("q-gate v{}", env!("CARGO_PKG_VERSION"));

    let analyzers = (JavaAnalyzer::new(), KotlinAnalyzer::new());
    let rules: Vec<Box<dyn Rule>> = vec![
        Box::new(GodClassRule::new(rule_config.god_class)),
        Box::new(LayerRule::new(rule_config.layers)),
    ];

    let detected_aspects = detect_android_aspects(&command_root);
    let aspects = all_aspects()
        .into_iter()
        .map(|aspect| {
            let config = merge_aspect_config(
                configured_aspects.get(&aspect),
                detected_aspects.get(&aspect),
            );
            (aspect, config)
        })
        .collect::<HashMap<_, _>>();

    let source_paths = if let Some(path) = explicit_path {
        let mut paths = Vec::new();
        collect_source_paths(&path, &command_root, &path, &mut paths)?;
        paths
    } else if full_scope {
        let mut paths = Vec::new();
        collect_source_paths(&command_root, &command_root, &command_root, &mut paths)?;
        paths
    } else {
        println!("Analyzing changed files (git diff)...");
        git::get_changed_files_from(&command_root)?
            .into_iter()
            .filter(|path| !path_policy::has_skipped_component(path, &command_root))
            .collect()
    };
    let targets = LanguageTargets::from_config(&analysis.targets)?;
    let files = parse_files(&source_paths, &analyzers, targets, &command_root)?;

    let mut violations = Vec::new();
    let mut advisories = Vec::new();
    for file in &files {
        for rule in &rules {
            violations.extend(rule.check(&file.path, &file.code, &file.tree));
        }
    }

    let mut external_aspects = HashMap::new();
    for aspect in all_aspects() {
        let config = &aspects[&aspect];
        if !should_run_aspect(aspect, config, full_scope) {
            continue;
        }

        if is_native_aspect(aspect) && !has_command(config) {
            match aspect {
                QualityAspect::DeadCode => {
                    let rule = DeadCodeRule::new();
                    for file in &files {
                        add_native_findings(
                            aspect,
                            config,
                            &mut violations,
                            &mut advisories,
                            rule.check(&file.path, &file.code, &file.tree),
                        );
                    }
                }
                QualityAspect::Duplication => {
                    let file_refs = files
                        .iter()
                        .map(|file| (file.path.as_str(), file.code.as_str(), &file.tree))
                        .collect::<Vec<_>>();
                    let rule = DuplicationRule::new();
                    add_native_findings(
                        aspect,
                        config,
                        &mut violations,
                        &mut advisories,
                        rule.check_files(&file_refs),
                    );
                }
                _ => unreachable!(),
            }
        } else {
            external_aspects.insert(aspect, config.clone());
        }
    }

    let (external_violations, external_advisories) =
        Orchestrator::new().run_aspects(&external_aspects, &command_root);
    violations.extend(external_violations);
    advisories.extend(external_advisories);
    violations.retain(|violation| !baseline.contains(&violation.fingerprint));
    advisories.retain(|advisory| !baseline.contains(&advisory.fingerprint));

    println!(
        "\nAdvisories:\n{}",
        serde_json::to_string_pretty(&advisories)?
    );
    println!(
        "\nViolations:\n{}",
        serde_json::to_string_pretty(&violations)?
    );

    if !violations.is_empty() {
        std::process::exit(1);
    }
    println!("\nNo violations found. Quality gate passed!");
    Ok(())
}

fn all_aspects() -> [QualityAspect; 8] {
    [
        QualityAspect::Style,
        QualityAspect::Lint,
        QualityAspect::TypeCheck,
        QualityAspect::UnitTest,
        QualityAspect::IntegrationTest,
        QualityAspect::Build,
        QualityAspect::DeadCode,
        QualityAspect::Duplication,
    ]
}

fn merge_aspect_config(
    configured: Option<&AspectConfig>,
    detected: Option<&AspectConfig>,
) -> AspectConfig {
    let mut merged = detected.cloned().unwrap_or_default();
    if let Some(configured) = configured {
        if configured.command.is_some() {
            merged.command = configured.command.clone();
        }
        if configured.stage.is_some() {
            merged.stage = configured.stage;
        }
        if configured.timeout_secs.is_some() {
            merged.timeout_secs = configured.timeout_secs;
        }
        if configured.severity.is_some() {
            merged.severity = configured.severity;
        }
        if configured.enabled.is_some() {
            merged.enabled = configured.enabled;
        }
    }
    merged
}

fn is_native_aspect(aspect: QualityAspect) -> bool {
    matches!(aspect, QualityAspect::DeadCode | QualityAspect::Duplication)
}

fn default_stage(aspect: QualityAspect) -> Stage {
    if matches!(
        aspect,
        QualityAspect::Style
            | QualityAspect::Lint
            | QualityAspect::TypeCheck
            | QualityAspect::DeadCode
            | QualityAspect::Duplication
    ) {
        Stage::PreCommit
    } else {
        Stage::PrePush
    }
}

fn should_run_aspect(aspect: QualityAspect, config: &AspectConfig, full_scope: bool) -> bool {
    if config.enabled == Some(false) || config.severity == Some(Severity::Ignore) {
        return false;
    }
    if full_scope {
        return true;
    }
    config.stage.unwrap_or_else(|| default_stage(aspect)) == Stage::PreCommit
}

fn has_valid_gradle_wrapper(root: &Path) -> bool {
    let name = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    };
    safe_regular_file(&root.join(name), root)
}

fn find_project_root(path: Option<&Path>) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut starts = Vec::new();
    if let Some(path) = path {
        starts.push(if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        });
    }
    starts.push(cwd.clone());

    for start in starts {
        let mut candidate = if start.is_dir() {
            start
        } else {
            start.parent().unwrap_or(&start).to_path_buf()
        };
        let mut build_script_root = None;
        loop {
            if has_qgate_entry(&candidate) || has_valid_gradle_wrapper(&candidate) {
                return candidate;
            }
            if build_script_root.is_none()
                && (safe_regular_file(&candidate.join("build.gradle"), &candidate)
                    || safe_regular_file(&candidate.join("build.gradle.kts"), &candidate))
            {
                build_script_root = Some(candidate.clone());
            }
            if !candidate.pop() {
                break;
            }
        }
        if let Some(root) = build_script_root {
            return root;
        }
    }
    cwd
}

fn has_command(config: &AspectConfig) -> bool {
    config
        .command
        .as_deref()
        .is_some_and(|command| !command.trim().is_empty())
}

#[derive(Debug, Clone, Copy)]
struct LanguageTargets {
    java: bool,
    kotlin: bool,
}

impl LanguageTargets {
    fn from_config(targets: &[String]) -> Result<Self> {
        if targets.is_empty() {
            return Ok(Self {
                java: true,
                kotlin: true,
            });
        }

        let mut result = Self {
            java: false,
            kotlin: false,
        };
        for target in targets {
            match target.trim().to_ascii_lowercase().as_str() {
                "java" | ".java" => result.java = true,
                "kotlin" | ".kt" => result.kotlin = true,
                _ => {
                    return Err(anyhow!(
                        "Unsupported analysis target '{target}'. Expected 'java' or 'kotlin'."
                    ));
                }
            }
        }
        Ok(result)
    }

    fn includes(self, extension: &str) -> bool {
        match extension.to_ascii_lowercase().as_str() {
            "java" => self.java,
            "kt" => self.kotlin,
            _ => false,
        }
    }
}

fn parse_files(
    paths: &[PathBuf],
    analyzers: &(JavaAnalyzer, KotlinAnalyzer),
    targets: LanguageTargets,
    root: &Path,
) -> Result<Vec<ParsedFile>> {
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("Failed to resolve project root {}", root.display()))?;
    paths
        .iter()
        .map(|path| {
            if contains_symlink_within_root(path, &canonical_root) {
                return Err(anyhow!(
                    "Source path {} contains a symlink; symlinks are not supported",
                    path.display()
                ));
            }
            let canonical_path = path.canonicalize().with_context(|| {
                format!("Failed to resolve source path {}", path.display())
            })?;
            if !canonical_path.starts_with(&canonical_root) {
                return Err(anyhow!(
                    "Source path {} is outside project root {}",
                    path.display(),
                    canonical_root.display()
                ));
            }
            if path_policy::has_skipped_component(&canonical_path, &canonical_root) {
                return Ok(None);
            }
            if !matches!(
                canonical_path.extension().and_then(|extension| extension.to_str()),
                Some(extension) if matches!(extension.to_ascii_lowercase().as_str(), "java" | "kt")
            ) {
                return Ok(None);
            }

            let metadata = fs::symlink_metadata(&canonical_path).with_context(|| {
                format!("Failed to inspect source file {}", canonical_path.display())
            })?;
            if !metadata.file_type().is_file() {
                return Err(anyhow!(
                    "Source path {} is not a regular file (symlinks and special files are not supported)",
                    path.display()
                ));
            }

            let extension = canonical_path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap();
            if !targets.includes(extension) {
                return Ok(None);
            }
            let code = fs::read_to_string(&canonical_path).with_context(|| {
                format!("Failed to read source file {}", canonical_path.display())
            })?;
            let tree = match extension.to_ascii_lowercase().as_str() {
                "java" => analyzers.0.analyze(&code)?,
                "kt" => analyzers.1.analyze(&code)?,
                _ => unreachable!(),
            };
            if tree.root_node().has_error() {
                return Err(anyhow!(
                    "Failed to parse source file {}: syntax errors found",
                    canonical_path.display()
                ));
            }
            Ok(Some(ParsedFile {
                path: relative_report_path(&canonical_root, &canonical_path),
                code,
                tree,
            }))
        })
        .filter_map(|result| result.transpose())
        .collect()
}

fn resolve_input_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if contains_symlink_within_root(&candidate, root) {
        return Err(anyhow!(
            "Analysis path {} contains a symlink; symlinks are not supported",
            path.display()
        ));
    }
    let candidate = candidate
        .canonicalize()
        .with_context(|| format!("Failed to resolve path {}", path.display()))?;
    if !candidate.starts_with(root) {
        return Err(anyhow!(
            "Analysis path {} is outside project root {}",
            path.display(),
            root.display()
        ));
    }
    if contains_symlink_within_root(&candidate, root) {
        return Err(anyhow!(
            "Analysis path {} contains a symlink; symlinks are not supported",
            path.display()
        ));
    }
    Ok(candidate)
}

fn relative_report_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn contains_symlink_within_root(path: &Path, root: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let mut current = PathBuf::new();
    let mut reached_root = false;
    for component in path.components() {
        current.push(component);
        if !reached_root {
            reached_root = current
                .canonicalize()
                .is_ok_and(|resolved| resolved == root);
            continue;
        }
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return true;
        }
    }
    false
}

fn safe_regular_file(path: &Path, root: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() || contains_symlink_within_root(path, &root) {
        return false;
    }
    path.canonicalize()
        .is_ok_and(|resolved| resolved.starts_with(&root))
}

fn has_qgate_entry(root: &Path) -> bool {
    fs::symlink_metadata(root.join("qgate.toml")).is_ok()
}

fn canonical_project_root(discovered: &Path) -> Result<PathBuf> {
    let root = discovered
        .canonicalize()
        .with_context(|| format!("Failed to resolve project root {}", discovered.display()))?;
    let metadata = fs::symlink_metadata(&root)
        .with_context(|| format!("Failed to inspect project root {}", root.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "Project root {} is not a regular directory",
            root.display()
        ));
    }

    let config = root.join("qgate.toml");
    if let Ok(metadata) = fs::symlink_metadata(&config)
        && (!metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || contains_symlink_within_root(&config, &root))
    {
        return Err(anyhow!(
            "Configuration {} must be a regular file with no symlinked path components",
            config.display()
        ));
    }
    Ok(root)
}

fn collect_source_paths(
    path: &Path,
    project_root: &Path,
    scan_root: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "Source path {} is a symlink; symlinks are not supported",
            path.display()
        ));
    }
    if metadata.is_dir() {
        if path != scan_root && path_policy::has_skipped_component(path, project_root) {
            return Ok(());
        }
        let mut entries = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let entry_path = entry.path();
            let Ok(entry_metadata) = fs::symlink_metadata(&entry_path) else {
                continue;
            };
            if entry_metadata.file_type().is_symlink()
                || !entry_metadata.is_dir() && !entry_metadata.is_file()
            {
                continue;
            }
            if entry_metadata.is_dir() {
                collect_source_paths(&entry_path, project_root, scan_root, paths)?
            } else {
                paths.push(entry_path);
            }
        }
    } else if metadata.is_file() {
        paths.push(path.to_path_buf());
    } else {
        return Err(anyhow!(
            "Source path {} is not a regular file or directory",
            path.display()
        ));
    }
    Ok(())
}

fn resolve_baseline_path(root: &Path, configured: &str) -> Result<PathBuf> {
    let configured = configured.trim();
    let path = Path::new(configured);
    if path.is_absolute() {
        return Err(anyhow!(
            "Baseline path must be relative to the project root"
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::CurDir
                | std::path::Component::RootDir
        )
    }) {
        return Err(anyhow!(
            "Baseline path must be a normalized relative path: {configured}"
        ));
    }
    let root = root.canonicalize()?;
    let candidate = root.join(path);
    let normalized = candidate
        .canonicalize()
        .with_context(|| format!("Failed to resolve baseline {}", configured))?;
    if !normalized.starts_with(&root) || contains_symlink_within_root(&candidate, &root) {
        return Err(anyhow!(
            "Baseline path must remain within the project root and contain no symlinks: {configured}"
        ));
    }
    Ok(candidate)
}

fn load_baseline(
    root: &Path,
    config: &BaselineConfig,
) -> Result<std::collections::HashSet<String>> {
    let mut fingerprints: std::collections::HashSet<String> = config
        .fingerprints
        .iter()
        .map(|fingerprint| fingerprint.trim().to_string())
        .filter(|fingerprint| !fingerprint.is_empty())
        .collect();
    let path = config
        .path
        .as_deref()
        .map(|path| resolve_baseline_path(root, path))
        .transpose()?
        .or_else(|| {
            let default = root.join("qgate-baseline.json");
            default.is_file().then_some(default)
        });

    if let Some(path) = path {
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("Failed to inspect baseline {}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(anyhow!(
                "Baseline {} is not a regular file (symlinks and special files are not supported)",
                path.display()
            ));
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read baseline {}", path.display()))?;
        let file = serde_json::from_str::<BaselineFile>(&content)
            .with_context(|| format!("Failed to parse baseline {}", path.display()))?;
        fingerprints.extend(
            file.fingerprints()
                .into_iter()
                .map(|fingerprint| fingerprint.trim().to_string())
                .filter(|fingerprint| !fingerprint.is_empty()),
        );
    }
    Ok(fingerprints)
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum BaselineFile {
    Object { fingerprints: Vec<String> },
    Array(Vec<String>),
}

impl BaselineFile {
    fn fingerprints(self) -> Vec<String> {
        match self {
            Self::Object { fingerprints } | Self::Array(fingerprints) => fingerprints,
        }
    }
}

fn add_native_findings(
    aspect: QualityAspect,
    config: &AspectConfig,
    violations: &mut Vec<Violation>,
    advisories: &mut Vec<config::Advisory>,
    findings: Vec<Violation>,
) {
    if config.severity == Some(Severity::Advisory) {
        advisories.extend(findings.into_iter().map(|finding| config::Advisory {
            aspect,
            message: finding.message,
            recommendation: format!("Fix the reported {aspect} finding."),
            fingerprint: finding.fingerprint,
        }));
        return;
    }

    violations.extend(findings.into_iter().map(|mut finding| {
        if matches!(config.severity, Some(Severity::Warn | Severity::Warning)) {
            finding.severity = "warning".to_string();
        }
        finding
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "qgate_main_test_{}_{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
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

    fn parse_test_paths(root: &Path, paths: &[PathBuf]) -> Result<Vec<ParsedFile>> {
        parse_files(
            paths,
            &(JavaAnalyzer::new(), KotlinAnalyzer::new()),
            LanguageTargets {
                java: true,
                kotlin: true,
            },
            root,
        )
    }

    #[test]
    fn parse_files_rejects_git_paths_outside_the_root() {
        let dir = TestDir::new();
        let root = dir.path().join("build");
        fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("outside.java");
        fs::write(&outside, "class Outside {}").unwrap();

        let error = match parse_test_paths(&root, &[outside]) {
            Ok(_) => panic!("outside source path was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("outside project root"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn parse_files_rejects_git_paths_through_intermediate_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = TestDir::new();
        let root = dir.path().join("project");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("Outside.java"), "class Outside {}").unwrap();
        symlink(&outside, root.join("linked")).unwrap();

        let error = match parse_test_paths(&root, &[root.join("linked/Outside.java")]) {
            Ok(_) => panic!("symlink source path was accepted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("contains a symlink"), "{error}");
    }

    #[test]
    fn source_collection_is_root_aware_about_excluded_directories() {
        let dir = TestDir::new();
        let root_named_build = dir.path().join("build");
        fs::create_dir_all(&root_named_build).unwrap();
        let root_file = root_named_build.join("Root.java");
        fs::write(&root_file, "class Root {}").unwrap();
        let mut root_paths = Vec::new();
        collect_source_paths(
            &root_named_build,
            &root_named_build,
            &root_named_build,
            &mut root_paths,
        )
        .unwrap();
        assert_eq!(root_paths, vec![root_file]);

        let project = root_named_build.join("project");
        fs::create_dir_all(project.join("nested/build")).unwrap();
        let parent_file = project.join("Parent.java");
        let kept_file = project.join("nested/Keep.java");
        fs::write(&parent_file, "class Parent {}").unwrap();
        fs::write(&kept_file, "class Keep {}").unwrap();
        fs::write(
            project.join("nested/build/Skipped.java"),
            "class Skipped {}",
        )
        .unwrap();
        let mut project_paths = Vec::new();
        collect_source_paths(&project, &project, &project, &mut project_paths).unwrap();

        assert!(project_paths.contains(&parent_file));
        assert!(project_paths.contains(&kept_file));
        assert!(
            !project_paths
                .iter()
                .any(|path| path.to_string_lossy().contains("nested/build"))
        );
    }
}
