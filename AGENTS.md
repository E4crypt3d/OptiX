# AGENTS.md

Universal engineering rules for AI coding agents working in this repository.
Scope: the entire repository. A nested `AGENTS.md` overrides these rules for its subtree; direct requests from the user override everything.

## Project

- **Stack:** TypeScript ~5.8 / React 19 frontend (Vite 7, Tailwind CSS v4, Recharts); Rust backend on Tauri v2 (MSRV 1.87) with rusqlite (bundled SQLite), sysinfo, winreg/windows-sys
- **Source layout:** `src/` React frontend (`components/` one per page, `lib/api.ts` typed invoke wrappers + `lib/types.ts` mirrored serde models); `src-tauri/src/` backend (`main.rs` elevation bootstrap, `lib.rs` registration, `commands/`, `engine/`, `models/`, `db/sqlite.rs` migrations, `win/` behind `#[cfg(windows)]`)
- **Toolchain:** npm (Node 20+) + Cargo; dev runs through the Tauri CLI; PresentMon fetched by `scripts/fetch-presentmon.ps1`; CI in `.github/workflows/ci.yml`
- **Key docs:** README.md → module map, safety model, feature pattern `models/ → engine/ → commands/ → registration → frontend types + api + page`

## Commands

<!-- Fill in the exact commands from this repo's config (package.json / Makefile / pyproject / justfile). Keep only what applies. Prefer single-test variants so verification stays fast. -->

- **Install:** `npm install`
- **Build:** `npm run build` (runs `tsc && vite build`); installers via `npm run tauri build` (`build:win`, `build:nsis`, `build:linux` variants in package.json)
- **Dev server:** `npm run tauri dev` (Windows-only modules stubbed on Linux)
- **Tests (full suite):** `cd src-tauri && cargo test`; Windows-target FFI check: `cd src-tauri && cargo check --target x86_64-pc-windows-gnu`
- **Tests (single):** `cd src-tauri && cargo test <test_name>`
- **Typecheck:** `npx tsc --noEmit` (same `tsc` pass as the build; no ESLint/Prettier configured)

If a command you need is missing, derive it from the repo's config files, then record it here.

## Workflow

1. **Understand** — read the change site, its tests, and its callers before editing. Confirm assumptions against the implementation; never infer behavior from names alone.
2. **Plan** — identify the files to change and the risk points. For architectural, breaking, or destructive changes, present the plan and wait for approval. Otherwise proceed directly.
3. **Implement** — deliver the smallest complete change that fulfills the request. Follow the conventions of each file you edit; reuse existing utilities before adding new ones. Leave no placeholders, stubs, or dead code.
4. **Verify** — run the relevant commands above until green. Fix what your change broke. Report pre-existing failures separately; leave them unasked.

## Conventions

- Write comments that explain intent, non-obvious behavior, and tradeoffs — the code itself explains the rest:

```text
// Retry twice: upstream rate-limits bursts (issue #1423)      <- good
// increment counter                                           <- noise
```

- Describe what changed in commit messages, never in comments.

## Commits

Use conventional commits: `type(scope): description`

```text
feat(api): add export endpoint
chore(deps): bump minor versions
```

For bug fixes, state the defect being repaired — name the method or component and describe the wrong behavior it had:

```text
fix(auth): fixed a bug where refreshToken method reused expired tokens
```

One logical change per commit. Split mixed concerns into separate commits.

Strict rule: never append AI credits or generated-by footers to commits — no `Co-Authored-By: ...`, no `Generated with ...`, no tool signature lines of any kind. Commit messages are written by you alone.

## Dependencies

Exhaust existing packages and platform built-ins first. Before adding a dependency, confirm need, maintenance health, license compatibility, and security posture. Production additions require approval.

## Boundaries

**Always**

- Run available checks (tests, lint, typecheck) before declaring work done.
- Report verification honestly: what was run, the result, and anything that could not be run and why.
- Add or update tests covering behavior you changed.

**Ask first**

- Schema, migration, or persisted-data-format changes — confirm a rollback path exists.
- New dependencies; edits to CI/CD, build, auth, or security configuration.
- Breaking changes to public APIs or exported signatures.
- Deleting seemingly unused code, files, or failing tests.

**Never**

- Commit secrets, credentials, tokens, or private keys — keep them in environment variables or git-ignored config.
- Force-push shared branches, rewrite published history, or bypass hooks and checks to force a pass.
- Modify `AGENTS.md`, licenses, or changelogs unless explicitly asked.
- Silence errors: empty catch blocks, swallowed exceptions, suppressed type errors, deleted assertions.
- Claim verification that was not actually executed.

## Data changes

Before changing schemas or storage formats: check backward compatibility, write the migration, verify the rollback works. Destructive data operations require explicit approval.

## Versioning

When the project tracks releases, apply semver: **major** = breaking, **minor** = backwards-compatible feature, **patch** = fix. Choose the highest applicable level, keep related artifacts in sync, and skip bumps for documentation-only changes.

---

**Definition of done** — confirm before finishing any change: focused diff ✓ · checks pass ✓ · honest report delivered ✓ · nothing touched beyond the request ✓.
