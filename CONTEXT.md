# q-gate Domain Model

## Core Concepts

### Quality Aspect
A standard domain of software quality verification enforced by q-gate:
- **Style**: Code formatting enforcement (e.g., `rustfmt`, `prettier`).
- **Lint**: Code style, safety, and static analysis (e.g., `clippy`, `eslint`).
- **TypeCheck**: Static type safety validation (e.g., `tsc`, `mypy`).
- **UnitTest**: Unit test suite execution and pass verification (e.g., `cargo test`, `jest`).
- **IntegrationTest**: System/integration test suite execution (e.g., `pytest`, integration test binaries).
- **Build**: Compilation and build asset verification (e.g., `cargo check`, `vite build`).
- **DeadCode**: Detection of unused functions, modules, or exports (e.g., `cargo-deadlinks`, `knip`).
- **Duplication**: Code repetition and clone detection (e.g., `jscpd`).
- **R8**: Android R8/ProGuard stability and keep-rule optimization validation (e.g., `scripts/r8_firewall.py`).

### Tool Provider
An external command, tool, or script discovered in the codebase or declared in `qgate.toml` that executes verification for a specific Quality Aspect. q-gate delegates execution to external Tool Providers when available, falling back to native AST rules (e.g. for DeadCode and Duplication) or issuing Advisories when external tools are absent.

### Advisory
A non-blocking quality finding generated during an Audit. Advisories include missing tooling, unavailable execution environments, and findings explicitly configured as non-blocking; they never prevent the Gate from proceeding.
_Avoid_: warning, ignored violation

### Audit
The process of evaluating the codebase against defined Rules and checking external Tool Providers across Quality Aspects. An Audit can be scoped (e.g., `diff` or `full`) and mapped to Git gate stages (e.g., `pre-commit` or `pre-push`).

### Rule
A specific architectural, structural, or quality constraint (e.g., God Class, Layering, or Quality Aspect Enforcement). Rules check internal AST patterns or verify Tool Provider health.

### Violation
A blocking quality finding produced when a Rule or Tool Provider check fails. It includes a project-root-relative location, a stable Finding Identity, an error message, and a severity level.
_Avoid_: issue, warning

### Gate
A point in the development lifecycle (typically a Git hook) where an Audit is executed. The outcome of the Gate determines whether the lifecycle step can proceed.

### Baseline
A snapshot of existing Violations and Advisories that are intentionally suppressed by their stable Finding Identity. A Baseline prevents known findings from blocking active development while allowing new regressions to surface.
_Avoid_: ignore list

### Hook
A native Git integration managed by q-gate that orchestrates Audit execution at Git lifecycle events.

### Finding Identity
The stable identity of a Violation or Advisory used for Baseline suppression. It is independent of display line numbers and remains stable when unrelated source lines move.
_Avoid_: line-based fingerprint

### Project Root
The repository directory from which q-gate loads configuration, discovers Tool Providers, resolves Baselines, and reports project-relative finding paths.
_Avoid_: working directory
