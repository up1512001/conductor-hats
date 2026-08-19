# Agent briefing: conductor-multi-account

Read this before changing anything here. The parent repository's
[AGENTS.md](../AGENTS.md) still applies: no AI attribution in git history,
`type/topic` branches, base `develop`, SHA-pinned actions. This file adds the
rules specific to this subproject.

## What this is

Two Claude or Codex accounts live at once in Conductor, one per workspace. Three
ways to drive it, on purpose:

| Where you drive it | Survives a Conductor update | Code |
|---|---|---|
| `conductor-acct` CLI | yes | `bin/`, `lib/` |
| `/account` in the chat | yes | `commands/account.md` |
| Injected in-app panel | **no**, re-apply after each release | `src/panel/`, `tools/` |

`docs/how-it-works.md` is the routing mechanism, `docs/account-panel.md` is what
the panel does, `docs/panel-internals.md` is how it attaches, and
`docs/patching-conductor.md` is what the app bundle does and does not allow.

## Hard rules

### No file over 300 lines

301 lines is a failure, not a judgement call. It applies to every source file:
shell, TypeScript, SCSS, Python, Markdown, tests.

Exempt, because they are not written by hand or are prose:

- build output under `dist/`
- `pnpm-lock.yaml`
- `LICENSE`
- Markdown under `docs/`, and the Markdown at the root

A file approaching the limit is a file doing more than one job. Split by
responsibility, not by line count: `lib/routes.sh` and `lib/keychain.sh`, never
`lib/conductor-acct-part-2.sh`.

### Everything in the folder that owns it

```
bin/            entrypoints only. conductor-acct is dispatch, the routers are
                the hot path and source bin/_resolve.sh directly
lib/            sourced shell libraries, one concern per file
src/panel/      TypeScript for the injected UI
src/panel/styles/  SCSS partials, one per group of elements
dist/           build output, committed but never edited by hand
tools/          patching and dev-app tooling (Python and shell)
test/           harness.sh plus one *.test.sh per area
docs/           prose
commands/       the /account slash command
```

`dist/account-ui.js` is committed on purpose. Patching a Conductor then needs no
JavaScript toolchain, which matters for anyone who only wants the panel. CI
rebuilds it and fails if it differs from the commit, so it cannot go stale.

New code goes in the folder that owns its concern, or a new folder gets added to
this table in the same commit. A file in the wrong place is a review comment.

### No personal information, ever

This is published. A test enforces both halves of the rule, stated positively so
the test itself carries no personal data:

- every example address sits on a domain RFC 2606 reserves for documentation:
  `example.com`, `example.org`, `example.net`, or `.test` / `.example` /
  `.invalid` / `.localhost`
- no path names a real account: use `~` or `/Users/you`

Also: no real workspace or repository names, no tokens, no keychain hashes from a
real machine, no third party's name where their team identifier makes the point.

Addresses are masked everywhere the UI shows one. If you add a place that shows
one, mask it there too, and add it to the test.

### One source of truth for state

`conductor-acct` owns all state. The panel and the chat command read it and write
through it; neither keeps its own copy. If the panel and the CLI can disagree
about anything, that is the bug.

The one deliberate duplication is the masking rule, because the panel cannot
shell out once per row. A test runs both implementations over the same cases and
fails if they differ. Any future duplication needs the same treatment.

### The router is on the hot path

`bin/claude-router` runs on **every agent spawn**. Therefore:

- POSIX shell, no runtime dependency, no build step
- fails open: all resolution runs in a subshell whose failure leaves the agent
  starting normally. A broken install must never stop someone working.
- no forks on the path where the answer is already known

Do not rewrite this in a language that needs a runtime or a build. The cost lands
on every spawn, and the fail-open property is the whole safety story.

### The panel must not be able to break the app

It is injected into a compiled bundle, so a thrown exception is somebody's white
screen. Rules that follow from that:

- find anchors by product copy, never by generated class name
- when an anchor is missing, fail to appear; never throw
- every entry point wrapped, every shell call `.catch`ed
- `node --check` (and typecheck, once TypeScript lands) in CI

## Tooling

- **pnpm only**, never npm or yarn, with `minimumReleaseAge: 10080`.
- **Shell** for `bin/` and `lib/`: the hot path, per above.
- **TypeScript + esbuild** for `src/panel/`, bundled to one self-contained IIFE.
  The injected artifact has to be a single script with no module loader, so many
  small sources plus a build step is the only way to keep the 300-line rule.
- **SCSS** for panel styles, compiled and inlined into the bundle at build time.
- **Python** for `tools/`: Mach-O parsing and the asset map, standard library
  only.
- **No Rust.** Matching the host application's language buys zero extra access,
  which `docs/patching-conductor.md` sets out with evidence. It would add a
  toolchain for the same output.

Build before patching:

```sh
pnpm install
pnpm build          # src/panel + styles.scss -> dist/account-ui.js
tools/patch-ui.py   # injects dist/account-ui.js
```

## Before opening a pull request

```sh
pnpm install
pnpm typecheck
pnpm build            # then commit dist/account-ui.js if it changed
test/run.sh
shellcheck -x --source-path=SCRIPTDIR \
  bin/conductor-acct bin/_resolve.sh bin/claude-router bin/codex-router \
  lib/*.sh test/run.sh test/harness.sh test/*.test.sh \
  install.sh tools/make-dev-conductor.sh tools/repersonalize.sh
```

Zero shellcheck findings is the bar; it exits non-zero on info notes too.

Then: add a `CHANGELOG.md` entry, and keep `CONDUCTOR_ACCT_VERSION` in step with
the version in the panel source. They ship together, so a skew is a bug. A test
asserts both.

## No current debt

Every source file is under 300 lines and in the folder that owns it, and a test
enforces both. There is no allowlist to add to: the next file over the limit fails
the suite.

The three files that used to break the rule were split rather than exempted:
`bin/conductor-acct` into `lib/*.sh` with dispatch left behind, the single
injected script into `src/panel/*.ts` plus SCSS partials, and `test/run.sh` into
`test/harness.sh` and one file per area.

## Escalate, don't improvise

- Destructive git operations on pushed branches, and anything touching secrets:
  ask first.
- `patch-ui.py` on `/Applications/Conductor.app`: refuse. There is a `--i-know`
  flag; it is not for agents to pass.
- Deleting a profile is the one irreversible operation here. The panel signs out
  and nothing more; `remove` stays a terminal command.
