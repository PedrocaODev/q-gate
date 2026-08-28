# Tasks: q-gate Core

- [ ] **Task 1: Infrastructure & Dependencies**
    - [x] Initialize Cargo project.
    - [x] Add `tree-sitter`, `tree-sitter-java`, `tree-sitter-kotlin`.
    - [x] Add `git2`, `serde`, `clap`, `anyhow`.

- [ ] **Task 2: Configuration Module**
    - [x] Define `Config` structs with Serde.
    - [x] Implement `Config::load()` for `qgate.toml`.

- [ ] **Task 3: Base Analyzer Engine**
    - [x] Define `LanguageAnalyzer` trait.
    - [x] Implement `JavaAnalyzer` and `KotlinAnalyzer` (Tree-sitter wrappers).

- [x] Task 4: God Class Rule
    - [x] Define `Rule` and `Violation` traits/structs.
    - [x] Implement `GodClassRule` with method counting and LOC calculation.
    - [x] Add unit tests for `GodClassRule`.

- [x] Task 5: Layering Rule
    - [x] Implement `LayerRule` for import analysis.
    - [x] Implement path-to-layer mapping logic.
    - [x] Add unit tests for `LayerRule`.

- [x] Task 6: Git Diff Integration
    - [x] Implement `GitScope` to retrieve changed files using `libgit2`.
    - [x] Integrate with `main` to filter files in "diff" mode.

- [x] Task 7: CLI & Reporting
    - [x] Finalize `clap` argument parsing.
    - [x] Implement JSON serialization for violations.
    - [x] Implement human-readable colored output.

- [x] Task 8: End-to-End Validation
    - [x] Create `tests/fixtures` with violation-prone Java/Kotlin code.
    - [x] Run `q-gate` against fixtures and verify JSON output.

