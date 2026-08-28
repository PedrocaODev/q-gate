# ADR 0003: External Tool Provider Orchestration for Quality Aspects

## Context
`q-gate` must enforce that software codebases adhere to essential quality standards across eight key aspects: `Style`, `Lint`, `TypeCheck`, `UnitTest`, `IntegrationTest`, `Build`, `DeadCode`, and `Duplication`. Building custom linters, compilers, and test runners into `q-gate` for every ecosystem is unmaintainable, duplicates existing mature tools, and risks lagging behind language standards.

## Decision
We will design `q-gate` as an **Orchestrator** of external **Tool Providers** rather than a standalone linter:
1. `q-gate` delegates execution to ecosystem-native tools (e.g. Gradle, `ktlint`, `detekt`, `spotless` for Android/Kotlin projects) auto-detected from project files or explicitly mapped in `qgate.toml`.
2. During an `Audit`, `q-gate` executes configured Tool Providers, captures stdout/stderr with a 60-second execution timeout, and converts non-zero exit codes into blocking `Violations`.
3. If no Tool Provider is detected or configured for a required Quality Aspect, `q-gate` issues an **`Advisory`** prompting the development team to introduce or configure missing tooling.
4. Android (Gradle/Kotlin/Java) will be the primary target ecosystem for auto-detection rules, followed by multi-ecosystem expansion (Rust, Node, Python).

## Alternatives Considered
- **In-process AST Analysis**: Parsing and analyzing source files directly within `q-gate`.
  - *Cons*: High maintenance burden, duplicate effort, and incomplete coverage compared to established ecosystem linters (Android Lint, Detekt, Clippy, ESLint).
- **Static Configuration-Only Inspection**: Checking for configuration files (e.g., `build.gradle.kts` presence) without running verification commands.
  - *Cons*: Does not guarantee that tests pass, builds succeed, or lint rules are actively passing.

## Consequences
- `q-gate` remains lightweight, fast, and language-agnostic.
- System must handle sub-process management safely (timeouts, output truncation, exit code capture).
- Missing tools can be baselined or set to `Advisory` mode to prevent blocking legacy adoption.
