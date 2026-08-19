# AGENTS.md

## Scope

These instructions apply to the entire repository unless a more specific `AGENTS.md` exists in a subdirectory.

This file defines universal engineering rules for AI agents working in this repository.

---

## Non-negotiable rules

- Do not modify `AGENTS.md` unless explicitly requested.
- Do not provide only analysis when the requested change can be implemented — proceed to implementation unless approval is required.
- Do not rewrite working code without a clear, justified reason.
- Do not add unnecessary dependencies.
- Do not introduce unnecessary complexity.
- Do not add AI-generated filler comments or documentation.
- Do not claim tests, builds, or verification were completed unless you actually performed them.
- Preserve existing behavior unless a change is intentional and documented.

---

## Repository understanding

Before making changes, understand the existing system.

Inspect relevant:

- Source code
- Configuration
- Documentation
- Dependencies
- Tests
- Build setup

Identify:

- Application type
- Main technologies
- Existing conventions
- Architecture patterns
- Code style

Do not assume technology choices from filenames alone — read the actual implementation.

Follow existing project patterns instead of introducing new ones.

---

## Implementation workflow

### Understand

Before editing:

- Read the relevant code.
- Check related files.
- Understand how the current implementation works.

### Plan

Determine:

- What needs to change.
- Which files are affected.
- Potential risks and side effects.

For large architectural changes or risky modifications:

- Explain the plan before implementation.
- Ask for approval before proceeding.

For normal changes, continue with implementation.

### Implement

When changing code:

- Keep changes focused and atomic.
- Match existing project style.
- Reuse existing utilities and patterns.
- Prefer simple, maintainable solutions.
- Fully implement the requested functionality.

Avoid:

- Unrelated refactoring.
- Placeholder or incomplete implementations.
- New architecture without justification.
- Removing existing functionality unnecessarily.

### Verify

After changes:

Run appropriate checks when available:

- Tests
- Type checks
- Builds
- Linters
- Formatters

Report:

- What was verified.
- Results.
- Any checks that could not be performed (and why).

---

## Dependencies

Before adding dependencies:

- Check existing packages first.
- Prefer built-in functionality when practical.
- Consider maintenance, security, license, and compatibility.

Do not add dependencies for trivial problems.

---

## Security

Never:

- Commit secrets, credentials, tokens, or private keys.
- Hardcode sensitive information.
- Disable security protections to bypass issues.
- Expose sensitive data in logs.

Consider:

- Input validation.
- Authentication and authorization.
- Permissions and least privilege.
- Data protection (encryption, masking).
- Secure defaults.

---

## Data changes

Before changing data structures or schemas:

Check:

- Existing data compatibility.
- Migration requirements.
- Backup requirements.
- Rollback options.

Never:

- Delete user data without approval.
- Make destructive changes blindly.

---

## Versioning

Follow semantic versioning:

- **Major**: breaking changes.
- **Minor**: new backwards-compatible features.
- **Patch**: bug fixes.

Rules:

- Use the highest applicable version level.
- Keep versions synchronized across related artifacts.
- Update release notes when required.
- Do not bump versions for documentation-only changes unless required.

---

## Git commits

Use conventional commits:

```text
type: description
```

Examples:

```text
feat: add export feature
fix: resolve startup crash
chore: update dependencies
refactor: simplify data handling
docs: update installation guide
```

Keep commits focused, atomic, and descriptive.

---

## Code comments

Comments should explain:

- Why something exists.
- Non-obvious behavior.
- Important tradeoffs or constraints.

Do not add comments that:

- Explain obvious code.
- Restate the implementation.
- Describe what was changed (use commit messages for that).
- Add AI-generated filler.

---

## Engineering principles

- Prefer correctness over speed.
- Prefer simple solutions over clever ones.
- Minimize unnecessary changes.
- Read before replacing.
- Reuse existing patterns.
- Consider edge cases.
- Avoid premature optimization.
- Avoid unnecessary abstraction.

---

## AI behavior

When modifying code:

- Keep changes production-quality.
- Do not inflate code size without value.
- Do not add artificial complexity.
- Do not hide errors or swallow exceptions.
- Do not ignore failing checks.
- Do not rewrite unrelated areas.

The goal is reliable software changes, not maximum output.
