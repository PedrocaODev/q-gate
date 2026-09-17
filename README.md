# q-gate

q-gate is a command-line interface tool written in Rust. The tool enforces code quality gates and architectural boundaries in Java and Kotlin codebases.

## Key Capabilities

### Architectural Rules
- **God Class Detection**: Identifies classes that exceed method count (`max_methods`) or line count (`max_loc`) thresholds.
- **Layering Constraint Validation**: Enforces unidirectional dependencies between architectural layers (for example, `ui` -> `domain` -> `data`).

### Quality Aspects and Tool Providers
The tool orchestrates nine quality domains:
- `Style`
- `Lint`
- `TypeCheck`
- `UnitTest`
- `IntegrationTest`
- `Build`
- `DeadCode`
- `Duplication`
- `R8`

The orchestrator detects Android Gradle tasks automatically when a `gradlew` wrapper exists in the repository. You can also configure custom shell commands with a 60-second default execution timeout.

### Native AST Fallbacks
q-gate contains built-in tree-sitter analyzers for `DeadCode` and `Duplication`. The engine executes native analyzers when external tools are not configured.
- **DeadCode**: Identifies unused private members and unused primary constructor properties.
- **Duplication**: Identifies structural token clones across source files.

### Baseline Suppression
You can suppress existing findings with stable fingerprints. Store baseline fingerprints in `qgate-baseline.json` or in the inline `baseline.fingerprints` list inside `qgate.toml`. The tool ignores matching violations during subsequent audits.

### Scope-Based Audits
The tool supports two audit scopes:
- **Fast Diff Audits** (`--scope diff`): Analyzes changed git files for pre-commit gates.
- **Full Codebase Audits** (`--scope full`): Analyzes all project files for pre-push gates.

## Installation and Build

### Prerequisites
- Rust compiler with cargo (2024 edition support)

### Instructions
Clone the repository and enter the directory:
```bash
git clone https://github.com/PedrocaODev/q-gate.git
cd q-gate
```

Build the release binary:
```bash
cargo build --release
```

Install the binary into your Cargo bin directory:
```bash
cargo install --path .
```

## Configuration

Create a `qgate.toml` configuration file in the project root:

```toml
[analysis]
targets = ["java", "kotlin"]
scope = "diff"

[rules.god_class]
max_methods = 30
max_loc = 500

[rules.layers]
ui = ["domain"]
domain = ["data"]
data = []

[aspects.style]
command = "./gradlew spotlessCheck"
stage = "pre-commit"
timeout_secs = 60
severity = "warning"
enabled = true

[aspects.lint]
command = "./gradlew lintDebug"
stage = "pre-commit"
severity = "error"
enabled = true

[aspects.type_check]
command = "./gradlew compileDebugKotlin"
stage = "pre-commit"
severity = "error"
enabled = true

[aspects.unit_test]
command = "./gradlew testDebugUnitTest"
stage = "pre-push"
severity = "error"
enabled = true

[aspects.integration_test]
command = "./gradlew connectedDebugAndroidTest"
stage = "pre-push"
severity = "advisory"
enabled = true

[aspects.build]
command = "./gradlew assembleDebug"
stage = "pre-push"
severity = "error"
enabled = true

[aspects.dead_code]
stage = "pre-commit"
severity = "error"
enabled = true

[aspects.duplication]
stage = "pre-commit"
severity = "warning"
enabled = true

[aspects.r8]
command = "./gradlew minifyReleaseWithR8"
stage = "pre-push"
severity = "error"
enabled = true

[baseline]
path = "qgate-baseline.json"
fingerprints = [
  "aspect:dead_code",
  "src/main/ui/MainActivity.kt:god_class:MainActivity:methods",
]
```

## CLI Usage

Audit changed files against the pre-commit gate:
```bash
q-gate --scope diff
```

Audit all files against the full gate:
```bash
q-gate --scope full
```

Audit a specific directory:
```bash
q-gate --path src/main/java
```

Audit a single source file:
```bash
q-gate --path src/main/kotlin/App.kt
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
