## Nanotation Feedback Workflow

### Overview
This section specifies the mandatory workflow for processing user feedback provided via the [ANNOTATION] marker.

### Primary Requirements
1. **Recursive Discovery**: Upon request to process feedback, you must perform a recursive search for the string "[ANNOTATION]" within the specified or implicit scope (file, directory, or project).
2. **Context Analysis**: Before implementation, evaluate at least 10 lines of context surrounding each marker to ensure accurate comprehension of the requested change.
3. **Implementation**: Execute the requested modifications according to the annotation text. All code must adhere to the existing project style and language-specific idioms.
4. **Verification**: Validate all changes through syntax checking and execution of relevant test suites
5. **Cleanup**: Once the changes are verified, you must remove the entire line containing the "[ANNOTATION]" marker. This includes removing any associated comment syntax (e.g., //, #, --).

## Execution Scope
- **File Scope**: Process only the explicitly named file.
- **Directory/Project Scope**: Use recursive search tools (grep, rg, fd) to identify all markers. Respect .gitignore boundaries and exclude build artifacts (e.g., target/ directory).

### Constraints
- **Markdown Syntax**: Identify and ignore "[ANNOTATION]" markers located within triple-backtick (```) code blocks in Markdown files.
- **Persistence**: Do not remove a marker until the implementation is fully completed and verified.
- **Ambiguity Handling**: If an instruction is ambiguous, do not perform experimental changes. Implement reachable parts and request clarification for the remainder.

## Reporting
Provide a concise summary upon completion, listing the files modified and the nature of the changes addressed.
