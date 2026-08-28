## Why

`q-gate` currently enforces code structure rules (such as `GodClass` and `Layering` checks) by parsing ASTs. However, repositories require broader quality enforcement—including formatting (`Style`), static analysis (`Lint`), type checking (`TypeCheck`), unit testing (`UnitTest`), integration testing (`IntegrationTest`), build verification (`Build`), unused code detection (`DeadCode`), and clone detection (`Duplication`).

Instead of re-implementing linters or test runners inside `q-gate`, `q-gate` should act as an **Orchestrator** of external Tool Providers (starting with Android/Gradle ecosystem defaults), while retaining native AST fallbacks for `DeadCode` and `Duplication`. When expected tooling is missing, `q-gate` will issue actionable `Advisories` to guide developers.

## What Changes

1. **Quality Aspect Framework**: Define eight core Quality Aspects (`Style`, `Lint`, `TypeCheck`, `UnitTest`, `IntegrationTest`, `Build`, `DeadCode`, `Duplication`) in `q-gate`.
2. **External Tool Provider Orchestration**: Auto-detect Gradle tasks (e.g. `assembleDebug`, `testDebugUnitTest`, `lintDebug`, `spotlessCheck`, `ktlintCheck`) for Android/Kotlin projects, with explicit overrides in `qgate.toml`.
3. **Subprocess Execution & Safety**: Execute Tool Provider commands with a default 60-second timeout, capture stdout/stderr, and map non-zero exit codes to `Violations`.
4. **Native AST Fallbacks**: Implement built-in `tree-sitter` analyzers for local `DeadCode` (unused private members & unused imports) and `Duplication` (sliding-window AST token hashing) when external plugins are absent.
5. **Advisories & Baselining**: Issue non-blocking `Advisories` when a Quality Aspect lacks a configured Tool Provider. Allow suppressing Advisories and legacy Violations via `Baseline` or `qgate.toml`.

## Capabilities

### New Capabilities

- `quality-aspects`: Orchestration, detection, execution, and reporting of standard quality aspects across Tool Providers and native AST fallbacks.

### Modified Capabilities

- `cli-audit`: Enhanced audit reporting to display both blocking `Violations` and non-blocking `Advisories` across scope (`diff` and `full`) and gate stages (`pre-commit` and `pre-push`).

## Impact

- **Affected Systems**: `q-gate` CLI executable (`qgate.toml` configuration, `Audit` workflow, exit code behavior).
- **Risks**: External tool subprocess execution might stall or take longer than expected; mitigated by enforcing a strict 60s timeout per Tool Provider.
- **Scope Boundaries**: Initial auto-detection target is Android / Gradle projects, extensible to multi-ecosystem (Rust, Node, Python) in future updates.
