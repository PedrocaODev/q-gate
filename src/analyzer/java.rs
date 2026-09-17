use super::LanguageAnalyzer;
use anyhow::{Context, Result};
use tree_sitter::{Parser, Tree};

pub struct JavaAnalyzer {
    language: tree_sitter::Language,
}

impl JavaAnalyzer {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_java::LANGUAGE.into(),
        }
    }
}

impl LanguageAnalyzer for JavaAnalyzer {
    fn analyze(&self, code: &str) -> Result<Tree> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .context("Error loading Java grammar")?;
        parser
            .parse(code, None)
            .context("Failed to parse Java code")
    }
}
