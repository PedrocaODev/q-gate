## Implementation slices

### Slice 1: Configuration Schema & Aspect Types

**Tasks:** Tasks 1.1, 1.2
**Tests first:** Add unit tests in `src/config.rs` testing `qgate.toml` parsing for `[aspects.<name>]` tables and verifying default configuration values fail before implementing types.
**TDD exception:** none

### Slice 2: Android Tool Provider Auto-Detection & Subprocess Orchestration

**Tasks:** Tasks 2.1, 2.2
**Tests first:** Write unit tests in `src/aspects/android.rs` asserting Gradle task detection logic (`assembleDebug`, `testDebugUnitTest`, `lintDebug`, `compileDebugKotlin`, `spotlessCheck`) on mock build files, and unit tests in `src/aspects/orchestrator.rs` testing command execution and 60s timeout handling.
**TDD exception:** none

### Slice 3: Native AST Fallback Analyzers (DeadCode & Duplication)

**Tasks:** Tasks 3.1, 3.2
**Tests first:** Write unit tests in `src/rules/dead_code.rs` asserting detection of unused private functions and imports on Kotlin/Java code snippets, and unit tests in `src/rules/duplication.rs` asserting clone detection across code snippets.
**TDD exception:** none

### Slice 4: CLI Audit Integration & Output Formatting

**Tasks:** Tasks 4.1, 4.2
**Tests first:** Write CLI integration tests verifying `q-gate` output separating `Advisories` from `Violations` and verifying correct exit codes (`0` for advisories-only, `1` for violations).
**TDD exception:** none

## Review checkpoints

- After Slice 2: Review configuration parsing, Android Gradle task detection, and subprocess execution safety.
- After Slice 4: Full review of aspect orchestration, AST fallback rules, CLI integration, and exit code contracts via `/fix-loop`.

## Final verification intent

- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo fmt --check`
- `cargo build --release`

## Intended commit grouping

- Commit 1: `feat(config): add QualityAspect configuration schema and types`
- Commit 2: `feat(aspects): add Android Gradle auto-detection and subprocess orchestrator`
- Commit 3: `feat(rules): add native tree-sitter fallback analyzers for DeadCode and Duplication`
- Commit 4: `feat(cli): integrate quality aspect audit pipeline and advisory/violation reporting`
