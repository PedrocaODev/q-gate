# Design: q-gate Core

## Architecture

`q-gate` is structured as a modular Rust binary.

### Components

1.  **CLI Interface (`main.rs`)**: Handles argument parsing (`scope`, `path`, `output-format`) using `clap`.
2.  **Configuration (`config.rs`)**: Loads and validates `qgate.toml`.
3.  **Analyzer Engine (`analyzer/`)**:
    *   `LanguageAnalyzer` Trait: Defines the interface for multi-language support.
    *   `JavaAnalyzer` & `KotlinAnalyzer`: Implementations using `tree-sitter-java` and `tree-sitter-kotlin`.
4.  **Rule Engine (`rules/`)**:
    *   `Rule` Trait: Defines how a rule checks code and returns violations.
    *   `GodClassRule`: Calculates method count and LOC from AST nodes.
    *   `LayerRule`: Analyzes `import` statements against defined package boundaries.
5.  **Git Integration**: Uses the `git2` crate to find changed files in the current worktree or between branches.

### Data Flow

1.  `main` loads `Config`.
2.  Determines files to analyze (Full scan or Git diff).
3.  For each file:
    *   Selects appropriate `LanguageAnalyzer`.
    *   Parses into AST.
    *   Runs all active `Rules`.
4.  Aggregates `Violations`.
5.  Outputs report in JSON (for agents) or colored text (for humans).

### "God Class" Detection Logic
- Use Tree-sitter queries to find all `method_declaration` (Java) or `function_declaration` (Kotlin) nodes within a `class_declaration`.
- Count top-level functions in Kotlin files if they aren't inside a class.
- Calculate LOC by measuring the line range of the class node.

### "Layering" Enforcement Logic
- Map file paths to layers based on `qgate.toml` (e.g., `src/ui/*` -> `ui`).
- Extract all `import` statements using Tree-sitter.
- Verify that each import target is allowed by the source layer's rules.
