use serde::{Deserialize, Serialize};
use tree_sitter::Tree;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub rule: String,
    pub message: String,
    pub severity: String,
    pub fingerprint: String,
}

pub trait Rule {
    fn check(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation>;
}

pub mod dead_code;
pub mod duplication;
pub mod god_class;
pub mod layers;
