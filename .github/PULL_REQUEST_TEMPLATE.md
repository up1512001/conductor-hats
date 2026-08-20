# What changed

<!-- One or two sentences. What is different after this lands. -->

# Why

<!-- The problem, not the solution. If it fixes an issue, "Fixes #123". -->

# How it was verified

<!--
Say what you ran, not that you were careful. Delete rows that do not apply.
-->

- [ ] `test/run.sh`
- [ ] `shellcheck -x --source-path=SCRIPTDIR test/*.sh install.sh tools/set-version.sh`
- [ ] `pnpm typecheck && pnpm build`
- [ ] `cargo build --release` with no warnings, and `test/run.sh` against it
- [ ] Patched a Conductor copy and used the panel: `hats repatch`

Anything exercised by hand rather than by a test, and anything left unexercised:

# Checklist

- [ ] No file over 300 lines
- [ ] Docblocks only, no `//` comments and no comments inside shell bodies
- [ ] No personal information: example addresses on reserved domains, paths as `~`
- [ ] `CHANGELOG.md` updated under `## Unreleased`
- [ ] Version left alone, or changed with `tools/set-version.sh`
- [ ] Docblocks only: no `//` comments, no comments inside shell bodies
