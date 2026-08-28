# Proposal: q-gate Core Implementation

## Problem
In large Java and Kotlin projects (especially in Android development), classes tend to grow into "God Classes" that handle too many responsibilities. Additionally, architectural boundaries between layers (e.g., UI, Domain, Data) are often breached, leading to high coupling and making automated refactoring by AI agents risky and complex.

## Solution
We propose `q-gate`, a high-performance CLI tool written in Rust. It utilizes `tree-sitter` for robust AST-based analysis of Java and Kotlin files. 
`q-gate` will:
1. Detect "God Classes" based on method count and lines of code (LOC).
2. Enforce architectural layering rules by analyzing import dependencies.
3. Output machine-readable JSON reports designed to be consumed by coding agents (like Gemini CLI) to trigger automated refactoring.
4. Run both locally (as a git hook) and in CI/CD pipelines.

## Goals
- Fast analysis using Tree-sitter.
- Configurable thresholds via `qgate.toml`.
- Support for Java and Kotlin.
- Clear signaling for AI agents to perform refactoring.
- Git-aware analysis (diff-based) to focus on new changes.

## Non-Goals
- Full static analysis suite (not a replacement for SonarQube).
- Automatic fixing of issues (that's the job of the signaled agent).
- Support for languages other than Java/Kotlin in the MVP.
