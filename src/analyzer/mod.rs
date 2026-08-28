use anyhow::Result;
use tree_sitter::Tree;

pub mod java;
pub mod kotlin;

pub trait LanguageAnalyzer {
    fn analyze(&self, code: &str) -> Result<Tree>;
}
