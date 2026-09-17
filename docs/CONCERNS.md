# Architectural Concerns & Future Improvements

This document tracks metrics and architectural rules that we want to implement in `q-gate` beyond the MVP.

## Known Tooling Failures

- `cargo run -- --path /home/pedrogoncalves/projects/MotoAssistPremium` previously exited 1 before reporting findings because the target's `qgate.toml` uses `scope = "full"` and valid Kotlin at `ImeAwareEditText.kt:31` uses `fun interface`, which `tree-sitter-kotlin 0.3.5` parsed with an `ERROR` node. The target file is valid Kotlin; released `tree-sitter-kotlin 0.3.8` is also insufficient. The durable fix is to pin an upstream grammar revision containing the merged `fun interface` fix.
- `cargo test --test cli_audit` initially failed before exercising q-gate because stale Cargo test artifacts embedded `CARGO_BIN_EXE_q-gate=/home/pedrogoncalves/projects/q-gate/target/debug/q-gate`, while this checkout is `/home/pedrogoncalves/projects/moto-experiences-skills/tools/q-gate`. This is stale build output, not a source defect; a clean rebuild resolves it.
- `cargo run -- --path /home/pedrogoncalves/projects/MotoAssistPremium` next exited 1 at valid `data/src/main/java/com/motorola/data/lifecycle/lifecycle/AppLifecycleListener.kt:53`, the second consecutive `when` range entry `in AFTERNOON ->`. The pinned grammar suppressed automatic semicolon insertion before `in`; this is a valid-Kotlin/upstream-grammar incompatibility, not a target-source defect. Upstream commit `abce73ce4e6541c176f9bf8e488c4ef0eb96b25f` contains the fix and is descended from the pinned `fun interface` fix.
- `cargo run -- --path /home/pedrogoncalves/projects/MotoAssistPremium` next exited 1 at valid `presentation/motofour/src/main/java/com/motorola/moto/motofour/feature/mainactivity/MotoFourActivity.kt:146:17`, the ordinary `open()` call. The pinned grammar treated the soft keyword `open` only as an inheritance modifier, not as a callable `simple_identifier`; this is a valid-Kotlin/upstream-grammar incompatibility, not a target-source defect. Upstream commit `ec0da48c034c3e8a79bb88d7009f26250e2d56ca` contains the merged `simple_identifier` soft-keyword fix and retains the prior fixes.
- `cargo run -- --path /home/pedrogoncalves/projects/MotoAssistPremium` next exited 1 at valid `data/src/main/java/com/motorola/data/repository/experiences/facade/LoadExperiencesFacadeImpl.kt:74:33`, on `tipsFamilyProvider()?.also { family -> finalFamilyList.add(family) }`. This is valid Kotlin; the external scanner mishandled the `?.` safe-call token after an ordinary call with a trailing lambda. The upstream canonical merge fix is PR #267, commit `9b536dc8d8df5bef86f2f8775bf6146f7a61701d`, which makes `?.` a literal grammar token and retains the prior fixes.
- The `GoogleOneCardContentProvider.kt` parse failure was addressed first by fwcd commit `1852ea17b7f60fb3f9d84e0b1555d56b46b39fb1`, which fixed import and safe-call issues but regressed valid Compose function types such as `@Composable () -> Unit` because annotation-constructor precedence consumed `()`. q-gate now uses the package alias `tree-sitter-kotlin = { package = "brokk-tree-sitter-kotlin", version = "0.4.2" }`; Brokk 0.4.2 includes the newer valid-Kotlin fixes and scanner-based annotation-parenthesis disambiguation.

**Status / next action:** Focused regressions and all Rust checks pass. The full real `MotoAssistPremium` audit completed parsing and reached downstream findings/aspects without parse or syntax errors; remaining nonzero audit results are quality/environment findings, not parser failure.

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
