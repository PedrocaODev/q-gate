use super::{Rule, Violation};
use crate::config::GodClassConfig;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor, Tree};

pub struct GodClassRule {
    config: GodClassConfig,
}

impl GodClassRule {
    pub fn new(config: GodClassConfig) -> Self {
        Self { config }
    }

    fn check_java(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation> {
        let mut violations = Vec::new();
        let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();

        let class_query_str = "(class_declaration name: (identifier) @name) @class";
        let class_query = Query::new(&language, class_query_str).unwrap();
        let mut class_cursor = QueryCursor::new();
        let mut matches = class_cursor.matches(&class_query, tree.root_node(), code.as_bytes());

        while let Some(mat) = matches.next() {
            let class_node = mat.nodes_for_capture_index(1).next().unwrap();
            let class_name_node = mat.nodes_for_capture_index(0).next().unwrap();
            let class_name = &code[class_name_node.byte_range()];
            let start_line = class_node.start_position().row + 1;

            // Count methods in this class body
            let method_count = class_node
                .child_by_field_name("body")
                .map(|body| {
                    let mut cursor = body.walk();
                    body.children(&mut cursor)
                        .filter(|c| c.kind() == "method_declaration")
                        .count()
                })
                .unwrap_or(0);

            if method_count > self.config.max_methods {
                violations.push(Violation {
                    file: file_path.to_string(),
                    line: start_line,
                    rule: "god_class".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Class '{}' has {} methods (max {})",
                        class_name, method_count, self.config.max_methods
                    ),
                    fingerprint: format!("{}:god_class:{}:methods", file_path, class_name),
                });
            }

            let loc = class_node.end_position().row - class_node.start_position().row + 1;
            if loc > self.config.max_loc {
                violations.push(Violation {
                    file: file_path.to_string(),
                    line: start_line,
                    rule: "god_class".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Class '{}' spans {} lines (max {})",
                        class_name, loc, self.config.max_loc
                    ),
                    fingerprint: format!("{}:god_class:{}:loc", file_path, class_name),
                });
            }
        }

        violations
    }

    fn check_kotlin(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation> {
        let mut violations = Vec::new();
        let language: tree_sitter::Language = tree_sitter_kotlin::LANGUAGE.into();

        // Kotlin class declaration name is often a type_identifier
        let class_query_str = "(class_declaration ((type_identifier) @name)) @class";
        let class_query = Query::new(&language, class_query_str).unwrap();
        let mut class_cursor = QueryCursor::new();
        let mut matches = class_cursor.matches(&class_query, tree.root_node(), code.as_bytes());

        while let Some(mat) = matches.next() {
            let class_node = mat.nodes_for_capture_index(1).next().unwrap();
            let class_name_node = mat.nodes_for_capture_index(0).next().unwrap();
            let class_name = &code[class_name_node.byte_range()];
            let start_line = class_node.start_position().row + 1;

            // Count functions in this class body
            let method_count = (0..class_node.child_count())
                .map(|i| class_node.child(i).unwrap())
                .find(|c| c.kind() == "class_body")
                .map(|body| {
                    let mut cursor = body.walk();
                    body.children(&mut cursor)
                        .filter(|c| c.kind() == "function_declaration")
                        .count()
                })
                .unwrap_or(0);

            if method_count > self.config.max_methods {
                violations.push(Violation {
                    file: file_path.to_string(),
                    line: start_line,
                    rule: "god_class".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Class '{}' has {} methods (max {})",
                        class_name, method_count, self.config.max_methods
                    ),
                    fingerprint: format!("{}:god_class:{}:methods", file_path, class_name),
                });
            }

            let loc = class_node.end_position().row - class_node.start_position().row + 1;
            if loc > self.config.max_loc {
                violations.push(Violation {
                    file: file_path.to_string(),
                    line: start_line,
                    rule: "god_class".to_string(),
                    severity: "error".to_string(),
                    message: format!(
                        "Class '{}' spans {} lines (max {})",
                        class_name, loc, self.config.max_loc
                    ),
                    fingerprint: format!("{}:god_class:{}:loc", file_path, class_name),
                });
            }
        }

        violations
    }
}

impl Rule for GodClassRule {
    fn check(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation> {
        if file_path.ends_with(".java") {
            self.check_java(file_path, code, tree)
        } else if file_path.ends_with(".kt") {
            self.check_kotlin(file_path, code, tree)
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::LanguageAnalyzer;
    use crate::analyzer::java::JavaAnalyzer;
    use crate::analyzer::kotlin::KotlinAnalyzer;

    #[test]
    fn test_java_god_class_methods() {
        let config = GodClassConfig {
            max_methods: 2,
            max_loc: 100,
        };
        let rule = GodClassRule::new(config);
        let analyzer = JavaAnalyzer::new();
        let code = r#"
            class LargeClass {
                void m1() {}
                void m2() {}
                void m3() {}
            }
        "#;
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("Test.java", code, &tree);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("has 3 methods"));
    }

    #[test]
    fn test_java_god_class_loc() {
        let config = GodClassConfig {
            max_methods: 10,
            max_loc: 5,
        };
        let rule = GodClassRule::new(config);
        let analyzer = JavaAnalyzer::new();
        let code = "class LongClass {\n\n\n\n\n\n}";
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("Test.java", code, &tree);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("spans 7 lines"));
    }

    #[test]
    fn test_kotlin_god_class_methods() {
        let config = GodClassConfig {
            max_methods: 1,
            max_loc: 100,
        };
        let rule = GodClassRule::new(config);
        let analyzer = KotlinAnalyzer::new();
        let code = r#"
            class KotlinGod {
                fun f1() {}
                fun f2() {}
            }
        "#;
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("Test.kt", code, &tree);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("has 2 methods"));
    }

    #[test]
    fn test_kotlin_god_class_loc() {
        let config = GodClassConfig {
            max_methods: 10,
            max_loc: 3,
        };
        let rule = GodClassRule::new(config);
        let analyzer = KotlinAnalyzer::new();
        let code = "class LongKotlin {\n\n\n\n}";
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("Test.kt", code, &tree);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("spans 5 lines"));
    }

    #[test]
    fn test_nested_classes_methods() {
        let config = GodClassConfig {
            max_methods: 2,
            max_loc: 100,
        };
        let rule = GodClassRule::new(config);
        let analyzer = JavaAnalyzer::new();
        let code = r#"
            class Outer {
                void m1() {}
                class Inner {
                    void m2() {}
                    void m3() {}
                }
            }
        "#;
        let tree = analyzer.analyze(code).unwrap();
        let violations = rule.check("Test.java", code, &tree);
        // If it double counts, Outer will have 3 methods (m1, m2, m3) -> 1 violation
        // Inner will have 2 methods (m2, m3) -> 0 violations
        // Total expected: 0 if correctly scoped, 1 if double counting.
        // For now, let's see current behavior.
        assert_eq!(
            violations.len(),
            0,
            "Should not count methods in nested classes for the outer class"
        );
    }
}
