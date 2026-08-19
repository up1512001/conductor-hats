# What changed

<!-- One or two sentences. What is different after this lands. -->

# Why

<!-- The problem, not the solution. If it fixes an issue, "Fixes #123". -->

# How it was verified

<!--
Say what you ran, not that you were careful. Delete rows that do not apply.
-->

- [ ] `test/run.sh`
- [ ] `shellcheck -x --source-path=SCRIPTDIR bin/conductor-acct bin/_resolve.sh bin/*-router lib/*.sh test/*.sh install.sh`
- [ ] `pnpm typecheck && pnpm build`
- [ ] `cargo build --release && cargo clippy`
- [ ] Patched a Conductor copy and used the panel: `hats repatch`

Anything exercised by hand rather than by a test, and anything left unexercised:

# Checklist

- [ ] No file over 300 lines
- [ ] Docblocks only, no `//` comments and no comments inside shell bodies
- [ ] No personal information: example addresses on reserved domains, paths as `~`
- [ ] `CHANGELOG.md` updated under `## Unreleased`
- [ ] Version left alone, or changed with `tools/set-version.sh`
