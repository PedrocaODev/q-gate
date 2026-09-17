use super::LanguageAnalyzer;
use anyhow::{Context, Result};
use tree_sitter::{Parser, Tree};

pub struct KotlinAnalyzer {
    language: tree_sitter::Language,
}

impl KotlinAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_kotlin::LANGUAGE.into(),
        }
    }
}

impl LanguageAnalyzer for KotlinAnalyzer {
    fn analyze(&self, code: &str) -> Result<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .context("Error loading Kotlin grammar")?;
        parser
            .parse(code, None)
            .context("Failed to parse Kotlin code")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_fun_interface_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze(
                "class Container {\n    fun interface OnBackKeyListener {\n        fun onBack()\n    }\n}",
            )
            .expect("Kotlin should parse");

        assert!(!tree.root_node().has_error());
        assert!(tree.root_node().to_sexp().contains("class_declaration"));
    }

    #[test]
    fn parses_delegated_property_and_consecutive_when_ranges_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze(
                "class Lifecycle {\n    private val usageEvent: UsageEvent by inject()\n\n    fun lifecycle(hour: Int) {\n        when (hour) {\n            in MORNING -> Unit\n            in AFTERNOON -> Unit\n        }\n    }\n\n    companion object {\n        private val MORNING = 6..11\n        private val AFTERNOON = 12..17\n    }\n}",
            )
            .expect("Kotlin should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_open_as_an_ordinary_call_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze("fun navigate() {\n    open()\n}")
            .expect("Kotlin should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_safe_call_after_call_with_trailing_lambda_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze("fun load() {\n    provider()?.also { family -> families.add(family) }\n}")
            .expect("Kotlin should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_type_annotated_single_abstract_property_override_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze(
                "abstract class Provider : Base() {\n    @com.example.StorageLevel\n    abstract val requiredStorageLevel: String\n}",
            )
            .expect("Kotlin should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_content_provider_fixture_without_errors() {
        let code = std::fs::read_to_string("tests/fixtures/GoogleOneCardContentProvider.kt")
            .expect("Should read fixture");
        let tree = KotlinAnalyzer::new().analyze(&code).expect("Should parse");
        assert!(
            !tree.root_node().has_error(),
            "Content provider fixture has errors: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn parses_load_experiences_safe_navigation_fixture_without_errors() {
        let code = std::fs::read_to_string("tests/fixtures/LoadExperiencesFacadeImpl.kt")
            .expect("Should read fixture");
        let tree = KotlinAnalyzer::new().analyze(&code).expect("Should parse");
        assert!(
            !tree.root_node().has_error(),
            "Load experiences fixture has errors: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn parses_blazetheme_fixture_without_errors() {
        let code =
            std::fs::read_to_string("tests/fixtures/BlazeTheme.kt").expect("Should read fixture");
        let tree = KotlinAnalyzer::new().analyze(&code).expect("Should parse");
        assert!(
            !tree.root_node().has_error(),
            "Tree has error: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn parses_safe_cast_then_safe_nav_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze("val x = (res as? Success)?.data")
            .expect("Kotlin should parse");
        assert!(
            !tree.root_node().has_error(),
            "Safe cast + nav: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn parses_trailing_comma_in_nested_call_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze("fun f() { g(h(1, ), ) }")
            .expect("Kotlin should parse");
        assert!(
            !tree.root_node().has_error(),
            "Trailing comma: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn parses_composable_lambda_parameter_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze("fun BlazeTheme(content: @Composable () -> Unit) {}")
            .expect("Kotlin should parse");
        assert!(
            !tree.root_node().has_error(),
            "Composable parameter: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn parses_parenthesized_composable_lambda_parameter_without_errors() {
        let tree = KotlinAnalyzer::new()
            .analyze("fun BlazeTheme(content: (@Composable () -> Unit)) {}")
            .expect("Kotlin should parse");
        assert!(
            !tree.root_node().has_error(),
            "Parenthesized composable parameter: {}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn rejects_malformed_kotlin_source_with_error_nodes() {
        let tree = KotlinAnalyzer::new()
            .analyze("fun invalid( {")
            .expect("Should still produce a tree");
        assert!(tree.root_node().has_error());
    }
}
