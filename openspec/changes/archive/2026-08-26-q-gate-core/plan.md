# Plan: q-gate Core

## Slice 1: Finalizing God Class & Base Engine
- **Goal**: Ensure the base analysis engine is robust and the God Class rule is fully tested.
- **Tasks**: Task 4 (Tests), Task 7 (Partially).
- **Verification**: Run unit tests for `GodClassRule` with sample AST nodes.

## Slice 2: Architectural Layering
- **Goal**: Implement the layering enforcement rule.
- **Tasks**: Task 5.
- **Verification**: Run `q-gate` on a directory with known layering violations and check if they are caught.

## Slice 3: Git Integration
- **Goal**: Make the tool "Git-aware" to support fast local loops.
- **Tasks**: Task 6.
- **Verification**: Change a file, run `q-gate --scope diff`, and ensure only the changed file is analyzed.

## Slice 4: Polishing & Agent Signaling
- **Goal**: Ensure the JSON output is perfect for agent consumption and add human-friendly output.
- **Tasks**: Task 7 (Polishing), Task 8.
- **Verification**: Run `q-gate` on the entire project and verify the output format against `specs.md`.
