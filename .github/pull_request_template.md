## Summary

Describe the change and its user-visible behavior.

## Related bugs / issues

- Fixes #
- Regression test(s) added: `tests/regression/bug-XXX-*.test.ts` / `regression_bug_XXX_*` in Rust

## QA checklist

- [ ] Unit tests added or updated
- [ ] Integration tests added or updated where appropriate
- [ ] E2E tests added or updated where appropriate
- [ ] A failing regression test was written first for every fixed bug, and kept
- [ ] `npm run qa` passes locally
- [ ] `npm run test:e2e` passes, or the limitation is documented
- [ ] Regression suite passes (`npm run test:regression`)
- [ ] Manual test completed (`docs/MANUAL_TEST_CHECKLIST.md`, relevant sections)
- [ ] No known data-loss issue
- [ ] Browser tab safety checked: only tabs owned by the workspace can close
- [ ] App close/minimize behavior checked; no process is force-killed
- [ ] SQLite schema change has a migration and an upgrade test (or: no schema change)
- [ ] Documentation updated (README / docs)

## Release gate

- [ ] No open Critical bug
- [ ] No unresolved High-severity regression
