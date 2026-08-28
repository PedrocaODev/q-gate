# Review: enforce-quality-aspects

## Schema

- Review schema: v4
- Change: `enforce-quality-aspects`
- Review scope: cumulative implementation after Slices 1–4

## Verdict

**PASSED**

## Reviewed against

- `proposal.md`
- `design.md`
- `specs/quality-aspects/spec.md`
- `specs/cli-audit/spec.md`
- `tasks.md`
- `plan.md`

## Review loop

- Initial cumulative review: actionable findings identified and fixed.
- Fix pass: configuration-root loading, nested working-directory handling for Git diff paths, clippy cleanliness, and native multi-file dispatch were corrected.
- Final re-review: no actionable findings remain.

## Findings and dispositions

- Quality aspect enumeration and configuration: all eight aspects are represented; configured values override detected values field-by-field; disabled and ignored aspects are filtered.
- Advisories and violations: missing providers are non-blocking advisories; failed commands and native findings are blocking by default, with advisory/warning/ignore severity handling; CLI output separates `Advisories` and `Violations`, and exits 1 only when violations remain.
- Android detection: detection requires `gradlew`, scans root and nested Gradle build scripts, recognizes Android plugins, and maps the specified Debug tasks plus Spotless/Ktlint style providers.
- Root and configuration handling: audits launched from a nested directory now load the project-root `qgate.toml`, execute providers from that root, and use root-relative Git status paths.
- Stage filtering and scope validation: `diff`/`full`/`all` are validated; pre-commit defaults run fast aspects and full scope includes pre-push aspects; explicit paths are supported.
- Git diff handling: NUL-delimited status parsing handles renames/copies, skips deletions, and restricts parsed changed paths to Java/Kotlin files.
- Native AST behavior: DeadCode uses Java/Kotlin tree-sitter traversal for private members and imports with conservative annotation/wildcard handling; Duplication compares deterministic normalized token streams across files with 30-token or ten-line thresholds and stable fingerprints.
- Subprocess safety: stdout/stderr are read concurrently with bounded output, commands run with the configured timeout or 60-second default, Unix process groups are terminated on timeout, and exit/output are converted to findings without embedding command text in fingerprints.
- CLI reporting: reports are deterministic JSON sections with advisory-only runs passing and violation runs returning exit code 1.

## Verification

- `cargo test` — passed (40 unit tests and 3 integration tests).
- `cargo clippy -- -D warnings` — passed.
- `cargo fmt --check` — passed.
- `cargo build --release` — passed.
- Manual nested-directory diff audit — passed: changed Java paths resolve from the project root and native violations are reported.

No planning artifacts or task completion markers were modified.
