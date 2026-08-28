# ADR 0001: Baseline Fingerprinting for Legacy Violations

## Context
q-gate is often introduced to existing codebases with many pre-existing violations. Blocking all development until these are fixed is impractical. We need a way to track and ignore these "legacy" violations while still enforcing rules on new code.

## Decision
We will implement a "Baseline" feature. The original proposal for a
`--generate-baseline` flag is historical/planned; the current CLI does not
provide that command. Baselines are supplied through `qgate.toml` and are
maintained by users or automation.
1. The baseline stores a fingerprint for each violation or advisory. A fingerprint consists of the file path, rule ID, and a stable identity for the finding, not a display line or range.
2. During an audit, if a finding's fingerprint exists in the baseline, it will be suppressed from the final report and will not affect the gate.

## Alternatives Considered
- **Diff-Only Mode**: Only checking changed files. 
    - *Cons*: Doesn't catch regressions caused by changes in other files (e.g., a layering violation caused by a change in a dependency).
- **Inline Suppression Comments**: Adding `// q-gate-ignore` comments.
    - *Cons*: Pollutes the source code and is hard to manage at scale.

## Consequences
- Requires a storage format for the baseline (e.g., `qgate-baseline.json`).
- Baseline needs to be updated when violations are intentionally resolved.
- Fingerprints must be stable enough to survive unrelated code changes in the same file.
