use super::Violation;
use std::collections::{BTreeSet, HashMap};
use tree_sitter::{Node, Tree};

const MIN_TOKENS: usize = 30;
const MIN_LINES: usize = 10;

pub struct DuplicationRule;

#[derive(Clone)]
struct Token {
    id: u64,
    start_line: usize,
    end_line: usize,
}

struct TokenText {
    value: String,
    start_line: usize,
    end_line: usize,
}

#[derive(Clone)]
struct Match {
    left_start: usize,
    right_start: usize,
    length: usize,
}

impl DuplicationRule {
    pub fn new() -> Self {
        Self
    }

    /// Finds one maximal-enough clone per pair of files. Keeping the result at
    /// pair granularity prevents a long clone from producing dozens of
    /// overlapping reports.
    pub fn check_files(&self, files: &[(&str, &str, &Tree)]) -> Vec<Violation> {
        let raw_streams: Vec<(&str, Vec<TokenText>)> = files
            .iter()
            .map(|(path, code, tree)| (*path, tokens(tree.root_node(), code)))
            .collect();
        let values = raw_streams
            .iter()
            .flat_map(|(_, tokens)| tokens.iter().map(|token| token.value.clone()))
            .collect::<BTreeSet<_>>();
        let mut ids = HashMap::new();
        let mut occupied = HashMap::new();
        for value in values {
            let mut id = stable_value_hash(&value);
            while occupied
                .get(&id)
                .is_some_and(|existing: &String| existing != &value)
            {
                id = id.wrapping_add(1);
            }
            occupied.insert(id, value.clone());
            ids.insert(value, id);
        }
        let mut streams: Vec<(&str, Vec<Token>)> = raw_streams
            .into_iter()
            .map(|(path, tokens)| {
                (
                    path,
                    tokens
                        .into_iter()
                        .map(|token| Token {
                            id: ids[&token.value],
                            start_line: token.start_line,
                            end_line: token.end_line,
                        })
                        .collect(),
                )
            })
            .collect();
        // Canonical ordering keeps pair identity, fingerprints, and report order
        // independent of filesystem traversal/input order.
        streams.sort_by(|left, right| left.0.cmp(right.0));
        let mut findings = Vec::new();

        for left in 0..streams.len() {
            for right in left + 1..streams.len() {
                if let Some(matched) = find_match(&streams[left].1, &streams[right].1) {
                    let left_token = &streams[left].1[matched.left_start];
                    let right_token = &streams[right].1[matched.right_start];
                    let left_end = &streams[left].1[matched.left_start + matched.length - 1];
                    let right_end = &streams[right].1[matched.right_start + matched.length - 1];
                    let clone_hash = hash_tokens(
                        &streams[left].1[matched.left_start..matched.left_start + matched.length],
                    );
                    // ponytail: file-pair comparison remains O(F²); an index is deferred until scale requires it.
                    let fingerprint = format!(
                        "duplication:{}:{}:{}:{}",
                        streams[left].0, streams[right].0, clone_hash, matched.length
                    );
                    findings.push(Violation {
                        file: streams[left].0.to_string(),
                        line: left_token.start_line,
                        rule: "duplication".to_string(),
                        severity: "error".to_string(),
                        message: format!(
                            "Structural duplication between {} range {}-{} and {} range {}-{} ({} normalized tokens)",
                            streams[left].0,
                            left_token.start_line,
                            left_end.end_line,
                            streams[right].0,
                            right_token.start_line,
                            right_end.end_line,
                            matched.length
                        ),
                        fingerprint,
                    });
                }
            }
        }

        findings.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then(left.line.cmp(&right.line))
                .then(left.fingerprint.cmp(&right.fingerprint))
        });
        findings
    }
}

const HASH_BASE: u64 = 1_099_511_628_211;

struct RollingHashes {
    prefix: Vec<u64>,
    powers: Vec<u64>,
}

impl RollingHashes {
    fn new(tokens: &[Token]) -> Self {
        let mut prefix = Vec::with_capacity(tokens.len() + 1);
        let mut powers = Vec::with_capacity(tokens.len() + 1);
        prefix.push(0u64);
        powers.push(1u64);
        for token in tokens {
            prefix.push(
                prefix
                    .last()
                    .copied()
                    .unwrap()
                    .wrapping_mul(HASH_BASE)
                    .wrapping_add(stable_token_hash(token.id)),
            );
            powers.push(powers.last().copied().unwrap().wrapping_mul(HASH_BASE));
        }
        Self { prefix, powers }
    }

    fn range(&self, start: usize, end: usize) -> u64 {
        self.prefix[end].wrapping_sub(self.prefix[start].wrapping_mul(self.powers[end - start]))
    }
}

fn find_match(left: &[Token], right: &[Token]) -> Option<Match> {
    if left.len() >= MIN_TOKENS && right.len() >= MIN_TOKENS {
        let left_hashes = RollingHashes::new(left);
        let right_hashes = RollingHashes::new(right);
        let mut right_index: HashMap<u64, Vec<usize>> = HashMap::new();
        for start in 0..=right.len() - MIN_TOKENS {
            right_index
                .entry(right_hashes.range(start, start + MIN_TOKENS))
                .or_default()
                .push(start);
        }

        let mut best = None;
        for left_start in 0..=left.len() - MIN_TOKENS {
            let hash = left_hashes.range(left_start, left_start + MIN_TOKENS);
            let Some(right_starts) = right_index.get(&hash) else {
                continue;
            };
            for &right_start in right_starts {
                if !tokens_equal(left, left_start, right, right_start, MIN_TOKENS) {
                    continue;
                }
                let length = longest_common_span(
                    left,
                    right,
                    &left_hashes,
                    &right_hashes,
                    left_start,
                    right_start,
                );
                let candidate = Match {
                    left_start,
                    right_start,
                    length,
                };
                if best
                    .as_ref()
                    .is_none_or(|current: &Match| candidate.length > current.length)
                {
                    best = Some(candidate);
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }

    // A ten-line clone can contain fewer than 30 tokens. Index windows by
    // length and hash, then verify only matching candidates.
    let mut left_windows = Vec::new();
    let mut right_windows = Vec::new();
    line_windows(left, &mut left_windows);
    line_windows(right, &mut right_windows);
    let left_hashes = RollingHashes::new(left);
    let right_hashes = RollingHashes::new(right);
    let mut right_index: HashMap<(u64, usize), Vec<(usize, usize)>> = HashMap::new();
    for &(start, end) in &right_windows {
        right_index
            .entry((right_hashes.range(start, end), end - start))
            .or_default()
            .push((start, end));
    }
    for (left_start, left_end) in left_windows {
        let key = (
            left_hashes.range(left_start, left_end),
            left_end - left_start,
        );
        let Some(candidates) = right_index.get(&key) else {
            continue;
        };
        for &(right_start, _) in candidates {
            if tokens_equal(left, left_start, right, right_start, left_end - left_start) {
                return Some(Match {
                    left_start,
                    right_start,
                    length: left_end - left_start,
                });
            }
        }
    }
    None
}

fn longest_common_span(
    left: &[Token],
    right: &[Token],
    left_hashes: &RollingHashes,
    right_hashes: &RollingHashes,
    left_start: usize,
    right_start: usize,
) -> usize {
    let max_length = (left.len() - left_start).min(right.len() - right_start);
    let mut low = MIN_TOKENS;
    let mut high = max_length;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if left_hashes.range(left_start, left_start + middle)
            == right_hashes.range(right_start, right_start + middle)
            && tokens_equal(left, left_start, right, right_start, middle)
        {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    // Hashes only index candidates; token equality is the collision check.
    while low > MIN_TOKENS && !tokens_equal(left, left_start, right, right_start, low) {
        low -= 1;
    }
    if tokens_equal(left, left_start, right, right_start, low) {
        low
    } else {
        0
    }
}

fn tokens_equal(
    left: &[Token],
    left_start: usize,
    right: &[Token],
    right_start: usize,
    length: usize,
) -> bool {
    left[left_start..left_start + length]
        .iter()
        .zip(&right[right_start..right_start + length])
        .all(|(left, right)| left.id == right.id)
}

fn stable_value_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_token_hash(id: u64) -> u64 {
    id
}

fn line_windows(tokens: &[Token], windows: &mut Vec<(usize, usize)>) {
    let mut end = 0;
    for start in 0..tokens.len() {
        end = end.max(start);
        let minimum_end_line = tokens[start].start_line + MIN_LINES - 1;
        while end < tokens.len() && tokens[end].end_line < minimum_end_line {
            end += 1;
        }
        if end < tokens.len() {
            windows.push((start, end + 1));
        }
    }
}

fn tokens(node: Node<'_>, code: &str) -> Vec<TokenText> {
    let mut result = Vec::new();
    collect_tokens(node, code, &mut result);
    result
}

fn collect_tokens(node: Node<'_>, code: &str, result: &mut Vec<TokenText>) {
    let kind = node.kind();
    if is_comment(kind) {
        return;
    }
    // Literal parents own their contents (notably Java strings and Kotlin
    // strings), so normalize the whole value instead of leaking fragments.
    if is_literal(kind) {
        result.push(TokenText {
            value: "LITERAL".to_string(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        });
        return;
    }
    if node.child_count() == 0 {
        if !code[node.byte_range()].trim().is_empty() {
            result.push(TokenText {
                value: normalized_value(node, code),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
            });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tokens(child, code, result);
    }
}

fn normalized_value(node: Node<'_>, code: &str) -> String {
    let kind = node.kind();
    if is_literal(kind) {
        "LITERAL".to_string()
    } else if is_identifier(kind) {
        "IDENTIFIER".to_string()
    } else {
        code[node.byte_range()].to_string()
    }
}

fn is_comment(kind: &str) -> bool {
    kind == "comment" || kind.ends_with("_comment")
}

fn is_literal(kind: &str) -> bool {
    matches!(
        kind,
        "boolean_literal"
            | "bin_literal"
            | "character_literal"
            | "decimal_floating_point_literal"
            | "decimal_integer_literal"
            | "false"
            | "hex_floating_point_literal"
            | "hex_integer_literal"
            | "hex_literal"
            | "integer_literal"
            | "long_literal"
            | "null_literal"
            | "real_literal"
            | "string_literal"
            | "true"
            | "unsigned_literal"
    )
}

fn is_identifier(kind: &str) -> bool {
    kind == "identifier" || kind.ends_with("_identifier")
}

fn hash_tokens(tokens: &[Token]) -> String {
    // FNV-1a is small, deterministic, and avoids pulling in a hash crate for
    // a fingerprint that is only used to correlate identical source findings.
    let mut hash = 0xcbf29ce484222325u64;
    for token in tokens {
        for byte in token.id.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::DuplicationRule;
    use crate::analyzer::LanguageAnalyzer;
    use crate::analyzer::java::JavaAnalyzer;
    use crate::analyzer::kotlin::KotlinAnalyzer;
    use crate::rules::Violation;

    #[test]
    fn reports_a_structural_clone_across_java_files() {
        let first = r#"class First {
    void calculate() {
        int total = 0;
        total += 1;
        total += 2;
        total += 3;
        total += 4;
        total += 5;
        total += 6;
        System.out.println(total);
    }
}
"#;
        let second = r#"class Second {
    void process() {
        int total = 0;
        total += 1;
        total += 2;
        total += 3;
        total += 4;
        total += 5;
        total += 6;
        System.out.println(total);
    }
}
"#;
        let analyzer = JavaAnalyzer::new();
        let first_tree = analyzer.analyze(first).unwrap();
        let second_tree = analyzer.analyze(second).unwrap();
        let files = [
            ("First.java", first, &first_tree),
            ("Second.java", second, &second_tree),
        ];

        let findings: Vec<Violation> = DuplicationRule::new().check_files(&files);

        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.rule, "duplication");
        assert!(finding.file.contains("First.java"));
        assert!(finding.message.contains("First.java"));
        assert!(finding.message.contains("Second.java"));
        assert!(finding.message.contains("range"));
        assert!(!finding.fingerprint.is_empty());
    }

    #[test]
    fn ignores_a_single_file_and_short_blocks() {
        let code = r#"class OnlyOne {
    void shortBlock() {
        int total = 0;
        total += 1;
        total += 2;
    }
}
"#;
        let tree = JavaAnalyzer::new().analyze(code).unwrap();
        let files = [("OnlyOne.java", code, &tree)];

        let findings = DuplicationRule::new().check_files(&files);

        assert!(findings.is_empty());
    }

    #[test]
    fn accepts_thirty_tokens_even_when_the_clone_is_under_ten_lines() {
        let first = r#"class First { void run() {
        int a = 1; int b = 2; int c = 3; int d = 4;
        int e = 5; int f = 6; int g = 7; int h = 8;
        int i = 9; int j = 10;
    } }
"#;
        let second = first.replace("First", "Second");
        let analyzer = JavaAnalyzer::new();
        let first_tree = analyzer.analyze(first).unwrap();
        let second_tree = analyzer.analyze(&second).unwrap();
        let files = [
            ("First.java", first, &first_tree),
            ("Second.java", &second, &second_tree),
        ];

        assert_eq!(DuplicationRule::new().check_files(&files).len(), 1);
    }

    #[test]
    fn accepts_a_low_token_clone_that_spans_ten_lines() {
        let first = "class First {\n    void run() {\n        ;\n        ;\n        ;\n        ;\n        ;\n        ;\n        ;\n        ;\n        ;\n        ;\n    }\n}\n";
        let second = first.replace("First", "Second");
        let analyzer = JavaAnalyzer::new();
        let first_tree = analyzer.analyze(first).unwrap();
        let second_tree = analyzer.analyze(&second).unwrap();
        let files = [
            ("First.java", first, &first_tree),
            ("Second.java", &second, &second_tree),
        ];

        assert_eq!(DuplicationRule::new().check_files(&files).len(), 1);
    }

    #[test]
    fn literal_values_and_comments_do_not_change_structural_matching() {
        let first = r#"class First {
    void run() {
        String text = "first";
        char mark = 'a';
        boolean enabled = true;
        System.out.println(text);
        if (enabled) { System.out.println("message"); }
        text = text + "suffix";
    }
}
"#;
        let second = first
            .replace("First", "Second")
            .replace("first", "other")
            .replace("'a'", "'z'")
            .replace("true", "false")
            .replace("message", "different")
            .replace("suffix", "tail");
        let analyzer = JavaAnalyzer::new();
        let first_tree = analyzer.analyze(first).unwrap();
        let second_tree = analyzer.analyze(&second).unwrap();
        let files = [
            ("First.java", first, &first_tree),
            ("Second.java", &second, &second_tree),
        ];

        assert_eq!(DuplicationRule::new().check_files(&files).len(), 1);
    }

    #[test]
    fn findings_are_identical_for_reverse_input_order() {
        let first = r#"class First {
    void run() {
        int total = 0;
        total += 1; total += 2; total += 3;
        total += 4; total += 5; total += 6;
        total += 7; total += 8; total += 9;
        System.out.println(total);
    }
}
"#;
        let second = first.replace("First", "Second");
        let analyzer = JavaAnalyzer::new();
        let first_tree = analyzer.analyze(first).unwrap();
        let second_tree = analyzer.analyze(&second).unwrap();
        let ordered: [(&str, &str, &tree_sitter::Tree); 2] = [
            ("First.java", first, &first_tree),
            ("Second.java", second.as_str(), &second_tree),
        ];
        let reversed: [(&str, &str, &tree_sitter::Tree); 2] = [
            ("Second.java", second.as_str(), &second_tree),
            ("First.java", first, &first_tree),
        ];

        assert_eq!(
            DuplicationRule::new().check_files(&ordered),
            DuplicationRule::new().check_files(&reversed)
        );
    }

    #[test]
    fn kotlin_literal_values_are_normalized() {
        let first = r#"class First {
    fun run() {
        val text = "first"
        val mark = 'a'
        val enabled = true
        println(text)
        if (enabled) { println("message") }
        println(text + "suffix")
    }
}
"#;
        let second = first
            .replace("First", "Second")
            .replace("first", "other")
            .replace("'a'", "'z'")
            .replace("true", "false")
            .replace("message", "different")
            .replace("suffix", "tail");
        let analyzer = KotlinAnalyzer::new();
        let first_tree = analyzer.analyze(first).unwrap();
        let second_tree = analyzer.analyze(&second).unwrap();
        let files = [
            ("First.kt", first, &first_tree),
            ("Second.kt", second.as_str(), &second_tree),
        ];

        assert_eq!(DuplicationRule::new().check_files(&files).len(), 1);
    }
}
