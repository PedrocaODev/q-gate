# Quality Aspects Specification

## Purpose

Defines the core Quality Aspect taxonomy, external Tool Provider orchestration for Android/Gradle projects, and native AST fallback analysis in q-gate.

## ADDED Requirements

### Requirement: Standard Quality Aspect Model

q-gate SHALL support eight distinct Quality Aspects: `Style`, `Lint`, `TypeCheck`, `UnitTest`, `IntegrationTest`, `Build`, `DeadCode`, and `Duplication`. Each aspect represents a specific domain of software quality verification.

#### Scenario: Aspect Identification and Configuration
Given a repository containing a `qgate.toml` configuration file
When q-gate parses `qgate.toml`
Then each aspect entry MUST map to one of the eight standard Quality Aspects with configurable `command`, `stage`, `timeout`, and `severity`.

### Requirement: External Tool Provider Orchestration and Auto-Detection

When running in an Android / Gradle repository containing `build.gradle`, `build.gradle.kts`, or `./gradlew`, q-gate SHALL auto-detect standard Gradle tasks for missing aspect definitions.

#### Scenario: Gradle Task Auto-Detection
Given an Android repository with `./gradlew` and `build.gradle.kts`
When q-gate performs an Audit without explicit aspect commands in `qgate.toml`
Then q-gate SHALL auto-detect and map:
- `Build` to `./gradlew assembleDebug`
- `UnitTest` to `./gradlew testDebugUnitTest`
- `Lint` to `./gradlew lintDebug`
- `TypeCheck` to `./gradlew compileDebugKotlin`
- `Style` to `./gradlew spotlessCheck` or `./gradlew ktlintCheck` (or Advisory if unconfigured).

#### Scenario: Subprocess Execution Timeout and Error Handling
Given an auto-detected or user-configured Tool Provider command
When q-gate executes the command during an Audit
Then q-gate MUST capture stdout and stderr, enforce a default 60-second timeout, and report a `Violation` if the command exits with a non-zero code or times out.

### Requirement: Native AST Fallbacks for DeadCode and Duplication

When external static analysis plugins (such as Detekt or PMD) are not configured for `DeadCode` or `Duplication`, q-gate SHALL execute built-in tree-sitter AST fallback analyzers.

#### Scenario: Native Unused Private Member Detection
Given a Kotlin or Java source file containing an unreferenced `private` function or import statement
When `DeadCodeRule` runs as a fallback analyzer
Then q-gate MUST report a `Violation` identifying the unreferenced identifier, file path, and line number.

#### Scenario: Native Code Duplication Detection
Given multiple Java or Kotlin source files containing identical code blocks of 10 lines or more
When `DuplicationRule` runs as a fallback analyzer
Then q-gate MUST report a `Violation` identifying the duplicate code fingerprint and the involved file paths and line ranges.
