# Architectural Layering Specification

## Purpose
This capability enforces architectural boundaries by restricting imports between defined project layers such as UI, Domain, and Data.

## ADDED Requirements

### Requirement: Import Direction Enforcement
The tool SHALL extract all import statements from a file and verify them against the `layers` configuration in `qgate.toml`.

#### Scenario: Illegal import detected
Given a file in the `domain` layer
And the configuration states `domain = []` (no allowed external dependencies)
When the file imports a class from the `ui` package
Then a violation MUST be reported specifying the source layer, the target package, and the forbidden dependency.

### Requirement: Path to Layer Mapping
The tool SHALL determine the layer of a file based on its path relative to the project root as defined in the configuration.

#### Scenario: Mapping file to UI layer
Given a file at `src/ui/MainActivity.java`
And `qgate.toml` defines layers by directory patterns
When `q-gate` analyzes the file
Then it MUST correctly identify the file as belonging to the `ui` layer for rule evaluation.
