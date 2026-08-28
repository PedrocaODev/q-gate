# God Class Detection Specification

## Purpose
This capability detects classes that have grown too large in terms of method count or lines of code, signifying a violation of the Single Responsibility Principle.

## ADDED Requirements

### Requirement: Method Count Threshold
The tool SHALL count the number of method declarations within a class and flag a violation if the count exceeds the configured `max_methods`.

#### Scenario: Class exceeds method limit
Given a Java class with 25 method declarations
And `max_methods` is set to 20
When `q-gate` analyzes the file
Then a violation MUST be reported with a message indicating the current count and the limit.

### Requirement: Lines of Code (LOC) Threshold
The tool SHALL calculate the lines of code occupied by a class declaration and flag a violation if the LOC exceeds the configured `max_loc`.

#### Scenario: Class exceeds LOC limit
Given a Kotlin class spanning 600 lines
And `max_loc` is set to 500
When `q-gate` analyzes the file
Then a violation MUST be reported with the line range and the specific limit breached.
