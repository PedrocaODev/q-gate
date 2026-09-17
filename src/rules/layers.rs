use super::{Rule, Violation};
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor, Tree};

pub struct LayerRule {
    layers: HashMap<String, Vec<String>>,
}

impl LayerRule {
    pub fn new(layers: HashMap<String, Vec<String>>) -> Self {
        Self { layers }
    }

    fn get_layer_for_path(&self, path: &str) -> Option<String> {
        path.split(['/', '.'])
            .find_map(|s| self.layers.get(s).map(|_| s.to_string()))
    }

    fn get_layer_for_import(&self, import: &str) -> Option<String> {
        import
            .split('.')
            .find_map(|s| self.layers.get(s).map(|_| s.to_string()))
    }
}

impl Rule for LayerRule {
    fn check(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation> {
        let mut violations = Vec::new();
        let source_layer = match self.get_layer_for_path(file_path) {
            Some(l) => l,
            None => return Vec::new(),
        };

        let allowed_deps = match self.layers.get(&source_layer) {
            Some(deps) => deps,
            None => return Vec::new(),
        };

        let query_str = if file_path.ends_with(".java") {
            "(import_declaration (scoped_identifier) @import)"
        } else if file_path.ends_with(".kt") {
            "(import_header (identifier) @import)"
        } else {
            return Vec::new();
        };

        let language = if file_path.ends_with(".java") {
            tree_sitter_java::LANGUAGE.into()
        } else {
            tree_sitter_kotlin::LANGUAGE.into()
        };

        let query = Query::new(&language, query_str).unwrap();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(mat) = matches.next() {
            let import_node = mat.nodes_for_capture_index(0).next().unwrap();
            let import_str = &code[import_node.byte_range()];

            if let Some(target_layer) = self.get_layer_for_import(import_str)
                && target_layer != source_layer
                && !allowed_deps.contains(&target_layer)
            {
                violations.push(Violation {
                    file: file_path.to_string(),
                    line: import_node.start_position().row + 1,
                    rule: "layering".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Layer violation: Layer '{}' is not allowed to import from layer '{}' (import: {})",
                        source_layer, target_layer, import_str
                    ),
                    fingerprint: format!("{}:layering:{}:{}", file_path, source_layer, target_layer),
                });
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::LanguageAnalyzer;
    use crate::analyzer::java::JavaAnalyzer;

    #[test]
    fn test_layer_violation_java() {
        let mut layers = HashMap::new();
        layers.insert("ui".to_string(), vec!["domain".to_string()]);
        layers.insert("domain".to_string(), vec![]);

        let rule = LayerRule::new(layers);
        let analyzer = JavaAnalyzer::new();

        // domain layer importing ui layer -> Violation
        let code = "package com.example.domain;\nimport com.example.ui.Widget;\nclass Service {}";
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("src/domain/Service.java", code, &tree);

        assert_eq!(violations.len(), 1);
        assert!(
            violations[0]
                .message
                .contains("Layer 'domain' is not allowed to import from layer 'ui'")
        );
    }

    #[test]
    fn test_layer_allowed_java() {
        let mut layers = HashMap::new();
        layers.insert("ui".to_string(), vec!["domain".to_string()]);

        let rule = LayerRule::new(layers);
        let analyzer = JavaAnalyzer::new();

        // ui layer importing domain layer -> OK
        let code = "package com.example.ui;\nimport com.example.domain.Model;\nclass Activity {}";
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("src/ui/Activity.java", code, &tree);

        assert_eq!(violations.len(), 0);
    }
}
