# Retrospective: enforce-quality-aspects

## What shipped

- Added enforcement for Style, Lint, TypeCheck, UnitTest, IntegrationTest, Build, DeadCode, and Duplication.
- Added qgate.toml aspect configuration, Android/Gradle auto-detection, stage and scope filtering, structured Advisory/Violation output, and stable baseline suppression.
- Added native tree-sitter DeadCode and Duplication analysis for Java and Kotlin.
- Added bounded subprocess output capture and timeout handling, including process-tree cleanup paths.
- Added project-root path normalization, symlink and special-file protection, generated/vendor directory exclusions, parse-error rejection, and configured language-target validation.
- Added focused regression coverage for configuration, paths, traversal, detection, dead-code resolution, duplication, baseline behavior, and CLI reporting.

## What went well

- The implementation was delivered in small, testable slices with focused regression tests.
- Repeated review passes found and removed security-sensitive path traversal cases and analysis false positives before archiving.
- The final verification commands passed: formatting, unit/integration tests, Clippy, and release build.
- The implementation avoided new dependencies and removed unused abstraction and dependency code.

## What to watch

- Native duplication detection retains an unavoidable O(F²) file-pair comparison ceiling.
- Android/Gradle detection intentionally prefers false negatives and Advisories over executing uncertain tasks.
- Windows-specific process and filesystem behavior should receive validation on a Windows runner; the local verification environment was Unix-based.
- Stable baseline identities should be checked when future rule output formats change.

## Follow-ups

- Update the design artifact to describe the implemented IntegrationTest no-device advisory condition rather than the earlier stage-based wording.
- Consider implementing baseline generation and native Git-hook management only if those planned capabilities become product requirements.
- Add broader cross-platform integration coverage when CI environments are available.
