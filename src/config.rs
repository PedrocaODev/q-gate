use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityAspect {
    Style,
    Lint,
    TypeCheck,
    UnitTest,
    IntegrationTest,
    Build,
    DeadCode,
    Duplication,
}

impl fmt::Display for QualityAspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QualityAspect::Style => write!(f, "style"),
            QualityAspect::Lint => write!(f, "lint"),
            QualityAspect::TypeCheck => write!(f, "type_check"),
            QualityAspect::UnitTest => write!(f, "unit_test"),
            QualityAspect::IntegrationTest => write!(f, "integration_test"),
            QualityAspect::Build => write!(f, "build"),
            QualityAspect::DeadCode => write!(f, "dead_code"),
            QualityAspect::Duplication => write!(f, "duplication"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    PreCommit,
    PrePush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warn,
    Warning,
    Advisory,
    Ignore,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warn | Severity::Warning => write!(f, "warning"),
            Severity::Advisory => write!(f, "advisory"),
            Severity::Ignore => write!(f, "ignore"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AspectConfig {
    pub command: Option<String>,
    pub stage: Option<Stage>,
    #[serde(alias = "timeout")]
    pub timeout_secs: Option<u64>,
    pub severity: Option<Severity>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advisory {
    pub aspect: QualityAspect,
    pub message: String,
    pub recommendation: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct BaselineConfig {
    pub path: Option<String>,
    #[serde(default)]
    pub fingerprints: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub analysis: AnalysisConfig,
    pub rules: RulesConfig,
    #[serde(default)]
    pub aspects: HashMap<QualityAspect, AspectConfig>,
    #[serde(default)]
    pub baseline: BaselineConfig,
}

#[derive(Debug, Deserialize)]
pub struct AnalysisConfig {
    pub targets: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
pub struct RulesConfig {
    pub god_class: GodClassConfig,
    pub layers: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct GodClassConfig {
    pub max_methods: usize,
    pub max_loc: usize,
}

impl Config {
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_aspects_config() {
        let toml_str = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]

            [aspects.style]
            command = "cargo fmt --check"
            enabled = true

            [aspects.lint]
            command = "cargo clippy"
            severity = "warn"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();

        assert!(config.aspects.contains_key(&QualityAspect::Style));
        let style = &config.aspects[&QualityAspect::Style];
        assert_eq!(style.command.as_deref(), Some("cargo fmt --check"));
        assert_eq!(style.enabled, Some(true));

        assert!(config.aspects.contains_key(&QualityAspect::Lint));
        let lint = &config.aspects[&QualityAspect::Lint];
        assert_eq!(lint.command.as_deref(), Some("cargo clippy"));
        assert_eq!(lint.severity, Some(Severity::Warn));
    }

    #[test]
    fn test_aspect_config_defaults() {
        let toml_str = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]

            [aspects.style]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let style = &config.aspects[&QualityAspect::Style];
        assert_eq!(style.command, None);
        assert_eq!(style.enabled, None);
        assert_eq!(style.severity, None);
        assert_eq!(style.stage, None);
        assert_eq!(style.timeout_secs, None);
    }

    #[test]
    fn test_stage_accepts_only_documented_values() {
        let base = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]

            [aspects.style]
            stage = "pre-push"
        "#;

        let config: Config = toml::from_str(base).unwrap();
        assert_eq!(
            config.aspects[&QualityAspect::Style].stage,
            Some(Stage::PrePush)
        );

        let invalid = base.replace("pre-push", "post-merge");
        assert!(toml::from_str::<Config>(&invalid).is_err());
    }

    #[test]
    fn test_unknown_aspect_config_field_is_rejected() {
        let toml_str = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]

            [aspects.style]
            unexpected = true
        "#;

        assert!(toml::from_str::<Config>(toml_str).is_err());
    }

    #[test]
    fn test_timeout_alias_is_accepted() {
        let toml_str = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]

            [aspects.build]
            timeout = 15
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.aspects[&QualityAspect::Build].timeout_secs, Some(15));
    }

    #[test]
    fn test_omitted_aspects_defaults_to_empty() {
        let toml_str = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.aspects.is_empty());
    }

    #[test]
    fn test_all_quality_aspects_and_full_config() {
        let toml_str = r#"
            [analysis]
            targets = ["src"]
            scope = "all"

            [rules.god_class]
            max_methods = 10
            max_loc = 100

            [rules.layers]

            [aspects.style]
            command = "cargo fmt"
            stage = "pre-commit"
            timeout_secs = 30
            severity = "warning"
            enabled = true

            [aspects.lint]
            [aspects.type_check]
            [aspects.unit_test]
            [aspects.integration_test]
            [aspects.build]
            [aspects.dead_code]
            [aspects.duplication]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.aspects.len(), 8);
        assert!(config.aspects.contains_key(&QualityAspect::Style));
        assert!(config.aspects.contains_key(&QualityAspect::Lint));
        assert!(config.aspects.contains_key(&QualityAspect::TypeCheck));
        assert!(config.aspects.contains_key(&QualityAspect::UnitTest));
        assert!(config.aspects.contains_key(&QualityAspect::IntegrationTest));
        assert!(config.aspects.contains_key(&QualityAspect::Build));
        assert!(config.aspects.contains_key(&QualityAspect::DeadCode));
        assert!(config.aspects.contains_key(&QualityAspect::Duplication));

        let style = &config.aspects[&QualityAspect::Style];
        assert_eq!(style.stage, Some(Stage::PreCommit));
        assert_eq!(style.timeout_secs, Some(30));
        assert_eq!(style.severity, Some(Severity::Warning));

        assert_eq!(QualityAspect::Style.to_string(), "style");
        assert_eq!(QualityAspect::TypeCheck.to_string(), "type_check");
        assert_eq!(QualityAspect::UnitTest.to_string(), "unit_test");
        assert_eq!(
            QualityAspect::IntegrationTest.to_string(),
            "integration_test"
        );
        assert_eq!(QualityAspect::Build.to_string(), "build");
        assert_eq!(QualityAspect::DeadCode.to_string(), "dead_code");
        assert_eq!(QualityAspect::Duplication.to_string(), "duplication");
    }
}
