# q-gate

A high-performance Rust CLI for enforcing code quality gates and architectural standards. It uses Tree-sitter to analyze Java and Kotlin codebases, detecting violations like "God Classes" and layering breaches to signal automated refactoring agents.

## Engineering Standards

- **Tech Stack**: Rust (Edition 2024).
- **Analysis Engine**: `tree-sitter` for grammar-based AST traversal. Support for `tree-sitter-java` and `tree-sitter-kotlin`.
- **Core Abstractions**:
    - **Analyzers (`src/analyzer/`)**: Language-specific AST mappings.
    - **Rules (`src/rules/`)**: Logic for detecting specific architectural violations.
- **Conventions**:
    - **Composition Over Inheritance**: Favor explicit delegation and wrapper types.
    - **Error Handling**: Use `anyhow` for CLI-level errors; preserve context in internal logic.
    - **Serialization**: `serde` for all JSON/TOML output to ensure agent compatibility.
    - **Formatting**: Strictly follow `cargo fmt`.
    - **Configuration compatibility**: `scope = "all"` aliases `full`; severity accepts both `warn` and `warning`.

## Baselines

An optional `[baseline]` section suppresses known fingerprints before output. Inline fingerprints are supported, and `path` is resolved from the project root. If `path` is omitted, `qgate-baseline.json` is loaded when present. Baseline files may be `{\"fingerprints\":[\"...\"]}` or a bare string array.

```toml
[baseline]
path = "qgate-baseline.json"
fingerprints = ["src/Foo.java:god_class:Foo:methods"]
```

## Agent Mandates

1. **Reproduction First**: Before fixing a rule or analyzer bug, create a fixture in `tests/fixtures/` that reproduces the failure.
2. **Surgical Edits**: Use `replace` for targeted logic changes. Avoid large `write_file` calls unless creating new rules or analyzers.
3. **Validation**: Every change must be verified by running the project tests: `cargo test`.
4. **Graph Maintenance**: If `graphify-out/` exists, run `graphify update .` after structural changes.

## Development Workflow

1. **Research**: Map AST nodes using `tree-sitter` queries to understand current detection logic.
2. **Strategy**: Propose rule logic or analyzer extensions before implementation.
3. **Execution**: Implement logic, add language fixtures, and update `Cargo.toml` if new grammars are needed.
4. **Verification**: 
    - `cargo check` for type safety.
    - `cargo test` for regression testing.
    - `q-gate --scope full --path <path>` to verify detection on local fixtures.
