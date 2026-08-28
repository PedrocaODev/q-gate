## Context

`q-gate` is currently a Rust CLI tool built with `clap`, `anyhow`, `serde`, `toml`, and `tree-sitter`. It checks rules like `GodClass` and `Layering` directly against Java and Kotlin ASTs. 

Repositories require enforcement across eight standard Quality Aspects: `Style`, `Lint`, `TypeCheck`, `UnitTest`, `IntegrationTest`, `Build`, `DeadCode`, and `Duplication`. Re-implementing linter engines and compilers inside `q-gate` is impractical and redundant. Instead, `q-gate` must orchestrate ecosystem-native tools (starting with Android/Gradle) while offering built-in tree-sitter fallbacks for local `DeadCode` and `Duplication`.

## Goals / Non-Goals

**Goals:**

- Define an explicit 8-aspect Quality Aspect model (`Style`, `Lint`, `TypeCheck`, `UnitTest`, `IntegrationTest`, `Build`, `DeadCode`, `Duplication`).
- Implement automatic Tool Provider detection for Android / Gradle repositories (`build.gradle`, `build.gradle.kts`, `./gradlew`).
- Support `qgate.toml` configuration overrides for explicit commands, execution stages (`pre-commit` vs `pre-push`), timeouts, and custom severity (`error`, `warning`, `advisory`, `ignore`).
- Implement subprocess execution with output capture and default 60s timeout per check.
- Implement native AST fallbacks for `DeadCode` (unused private functions/properties and unused imports) and `Duplication` (sliding-window AST token hashing) when external tools are absent.
- Provide clear Audit output separating blocking `Violations` from actionable non-blocking `Advisories`.

**Non-Goals:**

- Re-implementing external compilers or full static analyzer engines for Java/Kotlin/Android inside `q-gate`.
- Auto-installing missing Gradle plugins or system dependencies (q-gate advises; it does not mutate project build files without user consent).

## Decisions

### Decision 1: External Tool Provider Orchestration vs In-Process Analysis (ADR 0003)

`q-gate` delegates tool execution to standard external tools (`./gradlew assembleDebug`, `./gradlew testDebugUnitTest`, `./gradlew lintDebug`, `spotlessCheck`, `ktlintCheck`) auto-detected from project files or explicitly configured in `qgate.toml`. Subprocesses are spawned using `std::process::Command` with captured output and 60-second timeouts.

### Decision 2: Native AST Fallback Analyzers for DeadCode and Duplication

When external plugins (e.g. Detekt, PMD, jscpd) are absent, `q-gate` runs native `tree-sitter` AST analyzers:
- **`DeadCodeRule`**: Scans Java and Kotlin ASTs for unused `private` functions, unused `private` properties, and unused import declarations.
- **`DuplicationRule`**: Normalizes AST token streams across files and applies a sliding window hash scan (minimum 10 lines / 30 tokens) to catch identical structural blocks.

### Decision 3: Fast vs Heavy Gate Stage Assignment

To keep `pre-commit` fast (< 5s execution time), checks are partitioned by stage by default:
- **`pre-commit` stage (`--scope diff`)**: Fast checks (`Style`, `Lint`, `TypeCheck`, native `DeadCode`, native `Duplication`).
- **`pre-push` stage (`--scope full`)**: Heavy checks (`UnitTest`, `IntegrationTest`, `Build`, external tool suites).
Stage assignments can be explicitly overridden in `qgate.toml`.

### Decision 4: Advisory vs Violation Distinction

Non-zero exit codes from configured or auto-detected Tool Providers generate blocking `Violations` (causing `q-gate` to exit with code 1). Missing Tool Providers without native fallbacks produce non-blocking `Advisories` with clear recommendations on how to add or configure the tool.

## Risks / Trade-offs

- **Risk: Long-running or hung subprocesses**:
  - *Trade-off*: Default 60-second execution timeout per Tool Provider. Subprocesses exceeding the timeout are terminated and reported as a `Violation`.
- **Risk: Flaky or environment-dependent integration tests**:
  - *Trade-off*: `IntegrationTest` checks default to `Advisory` if no connected device/emulator is detected or if configured to `stage = "pre-push"`.
