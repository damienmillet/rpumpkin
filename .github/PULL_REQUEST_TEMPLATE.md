## Summary

- What does this PR change and why?

## Type of change

<!-- The CI requires at least one type label: bug, enhancement, documentation, release, ci, chore -->

- [ ] `bug` — something isn't working
- [ ] `enhancement` — new feature or request
- [ ] `documentation` — docs only
- [ ] `release` — version bump / release prep
- [ ] `ci` — CI / automation
- [ ] `chore` — maintenance

## Checklist

- [ ] A type label is set on this PR (checked by CI)
- [ ] `cargo fmt --check` and `cargo clippy --release -- -D warnings` pass (CI)
- [ ] WASM builds against both supported Pumpkin tags (CI `compat` job)
- [ ] README updated if the change is user-facing (CI checks the `rpumpkin-version` marker stays in sync with `Cargo.toml`)
- [ ] No secret or token committed
