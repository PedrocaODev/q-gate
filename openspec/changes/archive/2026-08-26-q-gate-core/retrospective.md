# Retrospective: q-gate Core Implementation

## What went well
- Tree-sitter 0.20 provided enough power for the MVP rules once dependencies were aligned.
- The modular architecture allowed easy addition of the `LayerRule`.
- TDD successfully caught a "double-counting" bug in nested classes.

## What could be improved
- Grammar loading is currently a bit "manual" due to 0.20 version constraints.
- Layer mapping heuristic is simple and could be more robust.
- The `git2` integration currently checks the whole status; it could be more precise about "new" vs "modified" if needed.

## Lessons Learned
- Always verify grammar version compatibility early.
- Tree-sitter field names are not guaranteed across all grammars (Kotlin lacked them for `class_declaration`).
