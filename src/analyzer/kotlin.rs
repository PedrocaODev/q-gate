use super::LanguageAnalyzer;
use anyhow::{Context, Result};
use tree_sitter::{Parser, Tree};

pub struct KotlinAnalyzer {
    language: tree_sitter::Language,
}

impl KotlinAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_kotlin::language(),
        }
    }
}

impl LanguageAnalyzer for KotlinAnalyzer {
    fn analyze(&self, code: &str) -> Result<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(self.language)
            .context("Error loading Kotlin grammar")?;
        parser
            .parse(code, None)
            .context("Failed to parse Kotlin code")
    }
}
