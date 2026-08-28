# CLI Audit Specification

## Purpose

Defines CLI Audit command options, Git gate stage filtering, and Advisory versus Violation report output formatting in q-gate.

## ADDED Requirements

### Requirement: Stage-Based Audit Scope Partitioning

q-gate SHALL support filtering checks by lifecycle stage (`pre-commit` vs `pre-push`) during Audits.

#### Scenario: Pre-Commit Fast Audit
Given a Git pre-commit hook invocation with `--scope diff`
When q-gate executes the Audit
Then q-gate SHALL run only fast checks (`Style`, `Lint`, `TypeCheck`, native `DeadCode`, native `Duplication`) and skip heavy test or build suites.

#### Scenario: Pre-Push Full Audit
Given a Git pre-push hook invocation or CLI run with `--scope full`
When q-gate executes the Audit
Then q-gate SHALL run all configured Tool Providers including `UnitTest`, `IntegrationTest`, and `Build`.

### Requirement: Advisory and Violation Reporting

q-gate SHALL output structured text separating non-blocking `Advisories` from blocking `Violations`.

#### Scenario: Missing Tool Advisory Generation
Given a repository where a Quality Aspect (e.g. `Style`) has no external Tool Provider detected or configured
When q-gate executes an Audit
Then q-gate MUST display a non-blocking `Advisory` recommending how to configure the missing tool, while keeping the gate exit code `0` if no `Violations` exist.

#### Scenario: Blocking Violation Exit Code
Given an Audit where at least one Tool Provider or native AST Rule fails
When q-gate completes the Audit run
Then q-gate MUST print the detailed `Violation` output and terminate with exit code `1`.
