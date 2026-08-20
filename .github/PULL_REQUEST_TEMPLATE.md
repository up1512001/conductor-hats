# What changed

<!-- One or two sentences. What is different after this lands. -->

# Why

<!-- The problem, not the solution. If it fixes an issue, "Fixes #123". -->

# How it was verified

<!--
Say what you ran, not that you were careful. Delete rows that do not apply.
-->

- [ ] `cargo test --all`
- [ ] `shellcheck -x --source-path=SCRIPTDIR install.sh`
- [ ] `pnpm typecheck && pnpm build`
- [ ] `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`
- [ ] Patched a Conductor copy and used the panel: `hats repatch`

Anything exercised by hand rather than by a test, and anything left unexercised:

# Checklist

- [ ] No file over 300 lines
- [ ] Docblocks only, no `//` comments and no comments inside shell bodies
- [ ] No personal information: example addresses on reserved domains, paths as `~`
- [ ] `CHANGELOG.md` updated under `## Unreleased`
- [ ] Version left alone, or changed with `cargo run --example set-version`
- [ ] Docblocks only: no `//` comments, no comments inside shell bodies
