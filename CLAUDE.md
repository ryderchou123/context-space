# Context Space: working rules

## QA is part of every change (permanent requirement)

For every feature, fix or refactor, without being asked:

1. Before implementing a major feature, write down the expected behavior and which existing features it can affect, and add test cases first.
2. Implement.
3. Add or update unit tests, integration tests where a boundary is crossed, and E2E where a user flow changes.
4. For every bug: reproduce it, write a test that fails because of it (`apps/desktop/tests/regression/bug-XXX-*.test.ts(x)` or Rust `fn regression_bug_XXX_*`), fix it, keep the test forever. Add it to `docs/QA_AUDIT.md`.
5. Run the full gate, not just the new tests: `npm run qa:full` (format, lint, typecheck, unit, integration, regression, Rust, build, E2E). Also `npm run build` for release-affecting changes.
6. Fix failures, then update README / `docs/` (QA_STRATEGY, MANUAL_TEST_CHECKLIST, QA_REPORT when a release is validated).
7. Report only results that were actually executed.

Details: `docs/QA_STRATEGY.md`.

## Safety invariants (never regress)

- Only close browser tabs owned by the workspace (`apps/extension/src/shared.ts` ownership rules). Never broaden URL matching; the spec is `apps/desktop/tests/fixtures/url-normalization.json` and Rust + both TS implementations must pass it.
- Never terminate a process. Minimize or `WM_CLOSE` only; browsers are never safe-closed.
- Every schema change is a new step appended to `MIGRATIONS` in `db.rs` plus an upgrade test. Never reset, recreate or silently repair the database.
- A stale active workspace after restart (`active_workspace_recovered`) must not trigger exit actions.
- An empty tab capture must not overwrite a Last Session.
- Logs stay local and never contain page URLs or titles.

## Environment notes

- `cargo` lives in `%USERPROFILE%\.cargo\bin` and is not on PATH by default; prepend it before `npm run qa`.
- After restoring Rust files with preserved timestamps, touch them or Cargo may reuse a stale build.
