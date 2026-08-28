# ADR 0002: Native Git Hook Management

## Context
The 'quality-gates' skill previously managed hooks using external tools like Lefthook or Husky. To make q-gate a self-contained engineering tool, we want to integrate this lifecycle management directly into the CLI.

## Decision
The original proposal for native hook management is historical/planned. The
current CLI does not provide an `install` or hook-audit command; repositories
invoke q-gate's existing audit CLI from their own hook setup.


## Alternatives Considered
- **Tool Integration**: Generating configs for Lefthook/Husky.
    - *Cons*: Adds a dependency on another tool being installed.
- **Manual Setup**: Just providing the audit command and letting users wire it.
    - *Cons*: Higher friction for users and less consistent behavior across a team.

## Consequences
- q-gate becomes the "one-stop shop" for quality gates in the repo.
- The CLI needs to be robust against different Git environments.
- We avoid the "red codebase" trap by checking for a baseline during the `install` process.
