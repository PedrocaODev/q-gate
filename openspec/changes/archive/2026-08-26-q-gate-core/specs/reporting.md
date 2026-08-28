# Reporting and Agent Signaling Specification

## Purpose
This capability provides machine-readable and human-readable feedback on quality gate violations to facilitate automated refactoring by AI agents.

## ADDED Requirements

### Requirement: JSON Output for Agents
The tool SHALL provide an option to output violations in a structured JSON format containing file path, rule name, message, line number, and severity.

#### Scenario: Generating JSON report
Given a session with multiple violations
When `q-gate` is executed with the default output format
Then it MUST print a JSON array of violation objects to standard output.

### Requirement: Non-Zero Exit Code on Violation
The tool SHALL exit with a non-zero status code if any violations are detected during analysis.

#### Scenario: CLI failure on violation
Given a file that violates the God Class rule
When `q-gate` completes its analysis
Then the process MUST exit with code 1 to signal a failure to the calling environment (e.g., a git hook or CI runner).
