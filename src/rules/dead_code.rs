use super::{Rule, Violation};
use tree_sitter::{Node, Tree};

pub struct DeadCodeRule;

#[derive(Clone)]
struct Declaration {
    name: String,
    symbol: String,
    range: (usize, usize),
    line: usize,
    kind: &'static str,
    required_arity: usize,
    total_arity: usize,
    vararg: bool,
}

struct LocalBinding {
    name: String,
    declaration_end: usize,
    scope: (usize, usize),
    parameter: bool,
}

struct Import {
    name: String,
    path: String,
    range: (usize, usize),
    line: usize,
}

impl DeadCodeRule {
    pub fn new() -> Self {
        Self
    }

    fn check_source(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation> {
        let java = file_path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("java"));
        let kotlin = file_path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("kt"));
        if !java && !kotlin {
            return Vec::new();
        }

        let mut declarations = Vec::new();
        let mut imports = Vec::new();
        collect_nodes(
            tree.root_node(),
            code,
            java,
            &mut declarations,
            &mut imports,
        );

        if declarations.is_empty() && imports.is_empty() {
            return Vec::new();
        }

        let declaration_ranges: Vec<(usize, usize)> =
            declarations.iter().map(|decl| decl.range).collect();
        let import_ranges: Vec<(usize, usize)> = imports.iter().map(|item| item.range).collect();
        let locals = collect_local_bindings(tree.root_node(), code);
        let mut used = std::collections::HashSet::new();
        collect_uses(
            tree.root_node(),
            code,
            java,
            &mut UseContext {
                declarations: &declarations,
                declaration_ranges: &declaration_ranges,
                import_ranges: &import_ranges,
                locals: &locals,
                used: &mut used,
            },
        );

        let mut violations = declarations
            .into_iter()
            .filter(|decl| !used.contains(&decl.symbol))
            .map(|decl| Violation {
                file: file_path.to_string(),
                line: decl.line,
                rule: "dead_code".to_string(),
                severity: "error".to_string(),
                message: format!("Unused private {} '{}'", decl.kind, decl.name),
                fingerprint: format!("{}:dead_code:{}:{}", file_path, decl.kind, decl.symbol),
            })
            .collect::<Vec<_>>();

        violations.extend(
            imports
                .into_iter()
                .filter(|item| !used.contains(&item.name))
                .map(|item| Violation {
                    file: file_path.to_string(),
                    line: item.line,
                    rule: "dead_code".to_string(),
                    severity: "error".to_string(),
                    message: format!("Unused import '{}'", item.path),
                    fingerprint: format!(
                        "{}:dead_code:import:{}:{}",
                        file_path, item.path, item.name
                    ),
                }),
        );

        violations.sort_by_key(|violation| violation.line);
        violations
    }
}

impl Rule for DeadCodeRule {
    fn check(&self, file_path: &str, code: &str, tree: &Tree) -> Vec<Violation> {
        self.check_source(file_path, code, tree)
    }
}

fn collect_nodes(
    node: Node<'_>,
    code: &str,
    java: bool,
    declarations: &mut Vec<Declaration>,
    imports: &mut Vec<Import>,
) {
    let kind = node.kind();

    if java {
        if kind == "import_declaration" {
            collect_java_import(node, code, imports);
        } else if kind == "method_declaration" && is_private(node, code) && !has_annotation(node) {
            if let Some(name) = node.child_by_field_name("name") {
                add_declaration(node, name, code, "function", declarations);
            }
        } else if kind == "field_declaration" && is_private(node, code) && !has_annotation(node) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "variable_declarator"
                    && let Some(name) = child.child_by_field_name("name")
                {
                    add_declaration(child, name, code, "member", declarations);
                }
            }
        }
    } else if kind == "import_header" {
        collect_kotlin_import(node, code, imports);
    } else if kind == "class_parameter" {
        if is_private(node, code)
            && !has_annotation(node)
            && is_class_property_parameter(node, code)
            && let Some(name) = direct_child_kind(node, "simple_identifier")
        {
            add_declaration(node, name, code, "member", declarations);
        }
    } else if (kind == "function_declaration" || kind == "property_declaration")
        && is_private(node, code)
        && !has_annotation(node)
    {
        if kind == "function_declaration" {
            // Kotlin 0.3 has no `name` field for this node. The first direct
            // simple_identifier is the declaration name, before parameters.
            if let Some(name) = direct_child_kind(node, "simple_identifier") {
                add_declaration(node, name, code, "function", declarations);
            }
        } else if let Some(variable) = direct_child_kind(node, "variable_declaration")
            && let Some(name) = direct_child_kind(variable, "simple_identifier")
        {
            add_declaration(node, name, code, "member", declarations);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, code, java, declarations, imports);
    }
}

fn add_declaration(
    owner: Node<'_>,
    name: Node<'_>,
    code: &str,
    kind: &'static str,
    declarations: &mut Vec<Declaration>,
) {
    let name_range = (name.start_byte(), name.end_byte());
    let name = code[name.byte_range()].to_string();
    let arity = declaration_arity(owner, code);
    let scope = containing_type(owner, code);
    declarations.push(Declaration {
        symbol: format!("{scope}:{kind}:{name}:{}", arity.total),
        name,
        range: name_range,
        line: owner.start_position().row + 1,
        kind,
        required_arity: arity.required,
        total_arity: arity.total,
        vararg: arity.vararg,
    });
}

fn collect_java_import(node: Node<'_>, code: &str, imports: &mut Vec<Import>) {
    if code[node.byte_range()].contains(".*") {
        // ponytail: wildcard imports are conservatively skipped; package/type
        // resolution is the upgrade path when cross-file symbol analysis exists.
        return;
    }
    let Some(path_node) = first_descendant_kind(node, "scoped_identifier")
        .or_else(|| first_descendant_kind(node, "identifier"))
    else {
        return;
    };
    let path = code[path_node.byte_range()].to_string();
    if let Some(name) = last_identifier(path_node, code) {
        imports.push(Import {
            name,
            path,
            range: (node.start_byte(), node.end_byte()),
            line: node.start_position().row + 1,
        });
    }
}

fn collect_kotlin_import(node: Node<'_>, code: &str, imports: &mut Vec<Import>) {
    if code[node.byte_range()].contains(".*") {
        // ponytail: wildcard imports are conservatively skipped; package/type
        // resolution is the upgrade path when cross-file symbol analysis exists.
        return;
    }
    let Some(path_node) = direct_child_kind(node, "identifier") else {
        return;
    };
    let path = code[path_node.byte_range()].to_string();
    let name = direct_child_kind(node, "import_alias")
        .and_then(|alias| first_descendant_kind(alias, "type_identifier"))
        .map(|alias| code[alias.byte_range()].to_string())
        .or_else(|| last_identifier(path_node, code));
    if let Some(name) = name {
        imports.push(Import {
            name,
            path,
            range: (node.start_byte(), node.end_byte()),
            line: node.start_position().row + 1,
        });
    }
}

struct UseContext<'a> {
    declarations: &'a [Declaration],
    declaration_ranges: &'a [(usize, usize)],
    import_ranges: &'a [(usize, usize)],
    locals: &'a [LocalBinding],
    used: &'a mut std::collections::HashSet<String>,
}

fn collect_local_bindings(node: Node<'_>, code: &str) -> Vec<LocalBinding> {
    let mut bindings = Vec::new();
    visit(node, &mut |candidate| {
        let parameter = matches!(
            candidate.parent().map(|parent| parent.kind()),
            Some(
                "formal_parameter"
                    | "spread_parameter"
                    | "receiver_parameter"
                    | "catch_formal_parameter"
                    | "parameter"
            )
        );
        let local_variable = candidate.parent().is_some_and(|parent| {
            (parent.kind() == "variable_declarator"
                && has_ancestor_kind(parent, "local_variable_declaration"))
                || (parent.kind() == "variable_declaration"
                    && has_ancestor_kind(parent, "property_declaration")
                    && has_ancestor_kind(parent, "function_body"))
        });
        if !is_identifier(candidate.kind()) || !(parameter || local_variable) {
            return;
        }
        let Some(scope) = lexical_scope(candidate) else {
            return;
        };
        bindings.push(LocalBinding {
            name: code[candidate.byte_range()].to_string(),
            declaration_end: candidate.end_byte(),
            scope: (scope.start_byte(), scope.end_byte()),
            parameter,
        });
    });
    bindings
}

fn lexical_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut ancestor = node.parent();
    while let Some(candidate) = ancestor {
        if matches!(
            candidate.kind(),
            "block" | "function_body" | "lambda_literal" | "catch_clause"
        ) {
            return Some(candidate);
        }
        ancestor = candidate.parent();
    }
    None
}

fn has_ancestor_kind(node: Node<'_>, kind: &str) -> bool {
    let mut ancestor = node.parent();
    while let Some(candidate) = ancestor {
        if candidate.kind() == kind {
            return true;
        }
        ancestor = candidate.parent();
    }
    false
}

fn has_descendant_kind(node: Node<'_>, kind: &str) -> bool {
    node.kind() == kind || {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .any(|child| has_descendant_kind(child, kind))
    }
}

fn collect_uses(node: Node<'_>, code: &str, java: bool, context: &mut UseContext<'_>) {
    if let Some((name, arity)) = called_symbol(node, code, java) {
        let scope = containing_type(node, code);
        let candidates = context
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.name == name
                    && declaration.kind == "function"
                    && declaration
                        .symbol
                        .starts_with(&format!("{scope}:function:{name}:"))
            })
            .collect::<Vec<_>>();
        let best_rank = candidates
            .iter()
            .filter_map(|declaration| overload_rank(declaration, arity))
            .max();
        for declaration in candidates {
            if best_rank.is_some_and(|rank| overload_rank(declaration, arity) == Some(rank)) {
                context.used.insert(declaration.symbol.clone());
            }
        }
    } else if is_identifier(node.kind())
        && !contains_range(context.import_ranges, node.start_byte(), node.end_byte())
        && !contains_range(
            context.declaration_ranges,
            node.start_byte(),
            node.end_byte(),
        )
        && !is_definition_identifier(node)
        && !is_call_callee(node, code, java)
        && !is_non_current_qualified_member_reference(node, code, java)
        && !is_shadowed_by_local(node, code, context.locals)
    {
        let name = code[node.byte_range()].to_string();
        context.used.insert(name.clone());
        let scope = containing_type(node, code);
        for declaration in context.declarations.iter().filter(|declaration| {
            declaration.name == name && declaration.symbol.starts_with(&format!("{scope}:"))
        }) {
            context.used.insert(declaration.symbol.clone());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_uses(child, code, java, context);
    }
}

fn called_symbol(node: Node<'_>, code: &str, java: bool) -> Option<(String, usize)> {
    if java && node.kind() == "method_invocation" {
        let name = node.child_by_field_name("name")?;
        if !java_receiver_is_supported(node, code) {
            return None;
        }
        let arguments = node.child_by_field_name("arguments")?;
        return Some((
            code[name.byte_range()].to_string(),
            arguments.named_child_count(),
        ));
    }
    if !java && node.kind() == "call_expression" {
        let (name, supported) = kotlin_call_name(node, code)?;
        if !supported {
            return None;
        }
        let arguments = first_descendant_kind(node, "value_arguments")
            .map(|arguments| arguments.named_child_count())
            .unwrap_or(0);
        return Some((name, arguments));
    }
    None
}

fn overload_rank(declaration: &Declaration, arity: usize) -> Option<u8> {
    if declaration.vararg {
        (arity >= declaration.required_arity).then_some(1)
    } else if declaration.total_arity == arity {
        Some(3)
    } else if declaration.required_arity <= arity && arity <= declaration.total_arity {
        Some(2)
    } else {
        None
    }
}

fn java_receiver_is_supported(node: Node<'_>, code: &str) -> bool {
    node.child_by_field_name("object").is_none_or(|receiver| {
        code[receiver.byte_range()].trim() == "this" || receiver.kind() == "this_expression"
    })
}

fn kotlin_call_name(node: Node<'_>, code: &str) -> Option<(String, bool)> {
    if let Some(name) = direct_child_kind(node, "simple_identifier") {
        return Some((code[name.byte_range()].to_string(), true));
    }
    let navigation = direct_child_kind(node, "navigation_expression")?;
    let suffix = direct_child_kind(navigation, "navigation_suffix")?;
    let name = first_descendant_kind(suffix, "simple_identifier")?;
    let receiver = navigation
        .named_child(0)
        .map(|receiver| code[receiver.byte_range()].trim())?;
    Some((
        code[name.byte_range()].to_string(),
        matches!(receiver, "this" | "self"),
    ))
}

fn is_call_callee(node: Node<'_>, _code: &str, java: bool) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if java && parent.kind() == "method_invocation" {
        return parent
            .child_by_field_name("name")
            .is_some_and(|name| same_range(name, node));
    }
    if !java && parent.kind() == "call_expression" && node.kind() == "simple_identifier" {
        return true;
    }
    !java
        && parent.kind() == "navigation_suffix"
        && has_ancestor_kind(parent, "call_expression")
        && node.kind() == "simple_identifier"
}

struct Arity {
    required: usize,
    total: usize,
    vararg: bool,
}

fn declaration_arity(node: Node<'_>, code: &str) -> Arity {
    let Some(parameters) = first_descendant_kind(node, "formal_parameters")
        .or_else(|| first_descendant_kind(node, "function_value_parameters"))
    else {
        return Arity {
            required: 0,
            total: 0,
            vararg: false,
        };
    };

    let parameter_nodes = direct_parameter_nodes(parameters);
    let mut required = 0;
    let mut vararg = false;
    for (index, parameter) in parameter_nodes.iter().enumerate() {
        let is_vararg = is_vararg_parameter(
            parameters,
            *parameter,
            parameter_nodes.get(index.wrapping_sub(1)).copied(),
            code,
        );
        vararg |= is_vararg;
        if !is_vararg
            && !has_default_value(
                parameters,
                *parameter,
                parameter_nodes.get(index + 1).copied(),
                code,
            )
        {
            required += 1;
        }
    }
    Arity {
        required,
        total: parameter_nodes.len(),
        vararg,
    }
}

fn is_vararg_parameter(
    parameters: Node<'_>,
    parameter: Node<'_>,
    previous_parameter: Option<Node<'_>>,
    code: &str,
) -> bool {
    if matches!(parameter.kind(), "spread_parameter" | "variadic_parameter")
        || has_descendant_kind(parameter, "ellipsis")
        || has_descendant_kind(parameter, "vararg")
    {
        return true;
    }
    let start = previous_parameter.map_or(parameters.start_byte(), |previous| previous.end_byte());
    code[start..parameter.start_byte()].contains("vararg")
}

fn direct_parameter_nodes<'a>(parameters: Node<'a>) -> Vec<Node<'a>> {
    let mut cursor = parameters.walk();
    parameters
        .children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "parameter" | "formal_parameter" | "spread_parameter" | "variadic_parameter"
            )
        })
        .collect()
}

fn has_default_value(
    parameters: Node<'_>,
    parameter: Node<'_>,
    next_parameter: Option<Node<'_>>,
    code: &str,
) -> bool {
    if has_descendant_kind(parameter, "default_value") {
        return true;
    }
    let end = next_parameter.map_or(parameters.end_byte(), |next| next.start_byte());
    code[parameter.end_byte()..end].contains('=')
}

fn containing_type(node: Node<'_>, code: &str) -> String {
    let mut types = Vec::new();
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if matches!(
            parent.kind(),
            "class_declaration" | "interface_declaration" | "object_declaration"
        ) {
            let name = parent
                .child_by_field_name("name")
                .or_else(|| first_descendant_kind(parent, "type_identifier"))
                .or_else(|| first_descendant_kind(parent, "simple_identifier"))
                .map(|name| code[name.byte_range()].to_string())
                .unwrap_or_else(|| "<anonymous>".to_string());
            types.push(name);
        }
        ancestor = parent.parent();
    }
    if types.is_empty() {
        "<top-level>".to_string()
    } else {
        types.reverse();
        types.join(".")
    }
}

fn is_private(node: Node<'_>, code: &str) -> bool {
    has_direct_modifier(node, code, "private")
}

fn has_annotation(node: Node<'_>) -> bool {
    let annotation_kinds = ["annotation", "annotation_entry", "marker_annotation"];
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|child| {
        annotation_kinds.contains(&child.kind())
            || (child.kind() == "modifiers" && {
                let mut modifiers = child.walk();
                child
                    .children(&mut modifiers)
                    .any(|modifier| annotation_kinds.contains(&modifier.kind()))
            })
    })
}

fn has_direct_modifier(node: Node<'_>, code: &str, modifier: &str) -> bool {
    let matches = |child: Node<'_>| {
        child.kind() == modifier
            || (child.kind() == "visibility_modifier"
                && code[child.byte_range()].trim() == modifier)
    };
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|child| {
        matches(child)
            || (child.kind() == "modifiers" && {
                let mut modifiers = child.walk();
                child.children(&mut modifiers).any(&matches)
            })
    })
}

fn is_shadowed_by_local(node: Node<'_>, code: &str, locals: &[LocalBinding]) -> bool {
    if is_explicit_member_reference(node) {
        return false;
    }
    let name = &code[node.byte_range()];
    locals.iter().any(|binding| {
        binding.name == name
            && node.start_byte() >= binding.scope.0
            && node.end_byte() <= binding.scope.1
            && (binding.parameter || node.start_byte() >= binding.declaration_end)
    })
}

fn is_explicit_member_reference(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() == "field_access" {
        return parent
            .child_by_field_name("field")
            .is_some_and(|field| same_range(field, node));
    }
    parent.kind() == "navigation_suffix"
}

fn is_non_current_qualified_member_reference(node: Node<'_>, code: &str, java: bool) -> bool {
    if java
        && node.parent().is_some_and(|parent| {
            parent.kind() == "field_access"
                && parent
                    .child_by_field_name("field")
                    .is_some_and(|field| same_range(field, node))
        })
    {
        let parent = node.parent().unwrap();
        let receiver = parent.child_by_field_name("object");
        return !receiver_is_current_type(receiver, code, node);
    }
    if !java
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "navigation_suffix")
    {
        let navigation = node.parent().and_then(|parent| parent.parent());
        let receiver = navigation.and_then(|navigation| navigation.named_child(0));
        return !receiver_is_current_type(receiver, code, node);
    }
    false
}

fn receiver_is_current_type(receiver: Option<Node<'_>>, code: &str, member: Node<'_>) -> bool {
    let Some(receiver) = receiver else {
        return false;
    };
    let receiver = code[receiver.byte_range()].trim();
    if matches!(receiver, "this" | "self") {
        return true;
    }
    let containing_type = containing_type(member, code);
    let current_type = containing_type.rsplit('.').next().unwrap_or_default();
    receiver == current_type
}

fn is_definition_identifier(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if matches!(
        parent.kind(),
        "variable_declarator"
            | "formal_parameter"
            | "spread_parameter"
            | "receiver_parameter"
            | "catch_formal_parameter"
    ) {
        return parent
            .child_by_field_name("name")
            .is_some_and(|name| same_range(name, node));
    }

    if parent.kind() == "method_declaration" || parent.kind() == "constructor_declaration" {
        return parent
            .child_by_field_name("name")
            .is_some_and(|name| same_range(name, node));
    }

    // Kotlin's grammar stores declaration names as direct children rather
    // than assigning them a `name` field.
    (matches!(
        parent.kind(),
        "function_declaration" | "variable_declaration"
    ) && node.kind() == "simple_identifier")
        || (matches!(parent.kind(), "parameter" | "class_parameter")
            && node.kind() == "simple_identifier")
        || (parent.kind() == "class_declaration" && node.kind() == "type_identifier")
}

fn same_range(left: Node<'_>, right: Node<'_>) -> bool {
    left.start_byte() == right.start_byte() && left.end_byte() == right.end_byte()
}

fn direct_child_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn is_class_property_parameter(node: Node<'_>, code: &str) -> bool {
    direct_child_kind(node, "val").is_some()
        || direct_child_kind(node, "var").is_some()
        || direct_child_kind(node, "binding_pattern_kind")
            .is_some_and(|kind| matches!(&code[kind.byte_range()], "val" | "var"))
}

fn first_descendant_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = first_descendant_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

fn last_identifier(node: Node<'_>, code: &str) -> Option<String> {
    let mut result = None;
    visit(node, &mut |child| {
        if is_identifier(child.kind()) {
            result = Some(code[child.byte_range()].to_string());
        }
    });
    result
}

fn is_identifier(kind: &str) -> bool {
    kind == "identifier" || kind.ends_with("_identifier")
}

fn contains_range(ranges: &[(usize, usize)], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|(range_start, range_end)| start >= *range_start && end <= *range_end)
}

fn visit<'a>(node: Node<'a>, callback: &mut impl FnMut(Node<'a>)) {
    callback(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit(child, callback);
    }
}

#[cfg(test)]
mod tests {
    use super::DeadCodeRule;
    use crate::analyzer::LanguageAnalyzer;
    use crate::analyzer::java::JavaAnalyzer;
    use crate::analyzer::kotlin::KotlinAnalyzer;
    use crate::rules::Rule;

    #[test]
    fn java_reports_unused_private_function_and_import() {
        let code = r#"import java.util.List;
class Example {
    private void unused() {
        int value = 1;
    }

    private int helper() {
        return 1;
    }

    void entry() {
        helper();
    }
}
"#;
        let tree = JavaAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.java", code, &tree);

        assert_eq!(violations.len(), 2);
        let unused = violations
            .iter()
            .find(|violation| violation.message.contains("unused"))
            .expect("unused private function should be reported");
        assert_eq!(unused.rule, "dead_code");
        assert_eq!(unused.file, "Example.java");
        assert_eq!(unused.line, 3);

        let import = violations
            .iter()
            .find(|violation| violation.message.contains("java.util.List"))
            .expect("unused import should be reported");
        assert_eq!(import.rule, "dead_code");
        assert_eq!(import.file, "Example.java");
        assert_eq!(import.line, 1);
    }

    #[test]
    fn kotlin_reports_unused_private_member_and_import() {
        let code = r#"import kotlin.collections.MutableList
class Example {
    private val unusedValue = 42
    private fun unusedFunction() {
        println("unused")
    }

    fun entry() {
        println("entry")
    }
}
"#;
        let tree = KotlinAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.kt", code, &tree);

        assert_eq!(violations.len(), 3);
        let unused_value = violations
            .iter()
            .find(|violation| violation.message.contains("unusedValue"))
            .expect("unused private property should be reported");
        assert_eq!(unused_value.rule, "dead_code");
        assert_eq!(unused_value.file, "Example.kt");
        assert_eq!(unused_value.line, 3);

        let unused_function = violations
            .iter()
            .find(|violation| violation.message.contains("unusedFunction"))
            .expect("unused private function should be reported");
        assert_eq!(unused_function.rule, "dead_code");
        assert_eq!(unused_function.file, "Example.kt");
        assert_eq!(unused_function.line, 4);

        let import = violations
            .iter()
            .find(|violation| violation.message.contains("kotlin.collections.MutableList"))
            .expect("unused import should be reported");
        assert_eq!(import.rule, "dead_code");
        assert_eq!(import.file, "Example.kt");
        assert_eq!(import.line, 1);
    }

    #[test]
    fn kotlin_counts_private_primary_constructor_properties_but_not_parameters() {
        let code = r#"class Example(private val unused: String, private val used: String, bare: String) {
    fun entry() {
        println(used)
        println(bare)
    }
}
"#;
        let tree = KotlinAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.kt", code, &tree);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("unused"));
    }

    #[test]
    fn local_or_parameter_shadowing_does_not_use_a_private_member() {
        let code = r#"class Example {
    private val value = 1

    fun entry(value: Int) {
        println(value)
    }

    fun actualMemberReference() {
        println(this.value)
    }
}
"#;
        let tree = KotlinAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.kt", code, &tree);

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn nested_local_only_shadows_references_inside_its_block() {
        let code = r#"class Example {
    private val value = 1

    fun entry() {
        println(value)
        run {
            val value = 2
            println(value)
        }
        println(value)
    }
}
"#;
        let tree = KotlinAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.kt", code, &tree);

        assert!(violations.is_empty());
    }

    #[test]
    fn kotlin_default_parameter_call_uses_private_function() {
        let code = r#"class Example {
    private fun helper(required: Int, optional: Int = 1) {}

    fun entry() {
        helper(1)
    }
}
"#;
        let tree = KotlinAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.kt", code, &tree);

        assert!(violations.is_empty());
    }

    #[test]
    fn java_vararg_call_uses_private_function() {
        let code = r#"class Example {
    private void helper(int... values) {}

    void entry() {
        helper(1, 2);
    }
}
"#;
        let tree = JavaAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.java", code, &tree);

        assert!(violations.is_empty());
    }

    #[test]
    fn annotated_java_private_methods_are_excluded() {
        let code = r#"class Example {
    @Deprecated
    private void legacy() {}
}
"#;
        let tree = JavaAnalyzer::new().analyze(code).unwrap();

        assert!(
            DeadCodeRule::new()
                .check("Example.java", code, &tree)
                .is_empty()
        );
    }

    #[test]
    fn wildcard_imports_are_conservatively_ignored() {
        let code = r#"import java.util.*;
class Example {}
"#;
        let tree = JavaAnalyzer::new().analyze(code).unwrap();

        assert!(
            DeadCodeRule::new()
                .check("Example.java", code, &tree)
                .is_empty()
        );
    }

    #[test]
    fn receiver_calls_only_use_same_type_methods() {
        let code = r#"class Example {
    private fun same() {}
    private fun other() {}
    fun entry() {
        this.same()
        self.other()
        external.same()
    }
}
"#;
        let tree = KotlinAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.kt", code, &tree);

        assert_eq!(violations.len(), 0, "this/self calls should count as uses");
    }

    #[test]
    fn qualified_external_properties_do_not_use_private_members() {
        let java = r#"class Example {
    private int value = 1;
    void entry() { external.value = 2; }
}
"#;
        let java_tree = JavaAnalyzer::new().analyze(java).unwrap();
        assert!(
            DeadCodeRule::new()
                .check("Example.java", java, &java_tree)
                .iter()
                .any(|violation| violation.message.contains("value"))
        );

        let kotlin = r#"class Example {
    private val value = 1
    fun entry() {
        external.value
    }
}
"#;
        let kotlin_tree = KotlinAnalyzer::new().analyze(kotlin).unwrap();
        assert!(
            DeadCodeRule::new()
                .check("Example.kt", kotlin, &kotlin_tree)
                .iter()
                .any(|violation| violation.message.contains("value"))
        );
    }

    #[test]
    fn current_type_qualified_properties_count_as_uses() {
        let java = r#"class Example {
    private int value = 1;
    void entry() { Example.value = 2; }
}
"#;
        let java_tree = JavaAnalyzer::new().analyze(java).unwrap();
        assert!(
            DeadCodeRule::new()
                .check("Example.java", java, &java_tree)
                .iter()
                .all(|violation| !violation.message.contains("value"))
        );

        let kotlin = r#"class Example {
    private val value = 1
    fun entry() {
        this.value
    }
}
"#;
        let kotlin_tree = KotlinAnalyzer::new().analyze(kotlin).unwrap();
        assert!(
            DeadCodeRule::new()
                .check("Example.kt", kotlin, &kotlin_tree)
                .iter()
                .all(|violation| !violation.message.contains("value"))
        );
    }

    #[test]
    fn same_named_private_methods_are_scoped_and_overloads_are_distinct() {
        let code = r#"class First {
    private void helper() {}
    private void helper(int value) {}
    void entry() { helper(); }
}
class Second {
    private void helper() {}
}
"#;
        let tree = JavaAnalyzer::new().analyze(code).unwrap();
        let violations = DeadCodeRule::new().check("Example.java", code, &tree);

        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|violation| violation.line == 3));
        assert!(violations.iter().any(|violation| violation.line == 7));
    }
}
