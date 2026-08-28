## 1. Domain Types & Configuration Schema

- [x] 1.1 Add `QualityAspect` enum, `AspectConfig`, `Advisory`, and `Severity` types to `src/config.rs` and `src/rules/mod.rs`.
- [x] 1.2 Implement unit tests verifying parsing of `qgate.toml` with aspect sections (`[aspects.style]`, `[aspects.lint]`, etc.).

## 2. External Tool Provider & Android Auto-Detection

- [x] 2.1 Create `src/aspects/mod.rs` and `src/aspects/android.rs` to auto-detect Gradle tasks for Android repositories (`assembleDebug`, `testDebugUnitTest`, `lintDebug`, `compileDebugKotlin`, `spotlessCheck`/`ktlintCheck`).
- [x] 2.2 Implement subprocess execution engine in `src/aspects/orchestrator.rs` with captured stdout/stderr, 60-second timeouts, exit code checking, and unit tests.

## 3. Native AST Fallback Rules (DeadCode & Duplication)

- [x] 3.1 Implement `DeadCodeRule` in `src/rules/dead_code.rs` using `tree-sitter` to detect unused private functions, properties, and imports in Java and Kotlin files, with unit tests.
- [x] 3.2 Implement `DuplicationRule` in `src/rules/duplication.rs` using normalized AST token sliding window hashing to detect code clones across Java and Kotlin files, with unit tests.

## 4. CLI Audit & Reporting Pipeline Integration

- [x] 4.1 Update `src/main.rs` to run aspect orchestration alongside rule checks, filtering by stage (`pre-commit` / `diff` vs `pre-push` / `full`).
- [x] 4.2 Update audit output to cleanly format `Advisories` (missing tooling recommendations) and `Violations` (failed checks), verifying exit codes in unit/integration tests.
