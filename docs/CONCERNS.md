# Architectural Concerns & Future Improvements

This document tracks metrics and architectural rules that we want to implement in `q-gate` beyond the MVP.

## Metrics & Rules

### God Classes
- [x] **Simple Count**: Total methods in a class. (MVP)
- [x] **LOC (Lines of Code)**: Total lines in a class/method. (MVP)
- [ ] **Weighted Complexity**: Methods weighted by internal logic (cyclomatic complexity, branching).
- [ ] **Threshold by Type**: Specific limits for Interfaces, Data Classes, Services, etc.

### Separation of Concerns
- [x] **Package Dependency Check**: Ensure layering (e.g., `ui` -> `domain` -> `data`). (MVP)
- [ ] **Circular Dependencies**: Detect and flag circular imports between packages.
- [ ] **Access Modifiers**: Enforce strict usage of `internal`, `private`, etc., based on architectural layers.

## Agent Integration
- [x] **Violation Flagging**: Provide file, line, and rule violated to the agent. (MVP)
- [ ] **Contextual Refactoring Suggestions**: Provide specific patterns (e.g., "Extract Delegate").
