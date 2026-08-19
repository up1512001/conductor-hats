# Agent briefing: conductor-multi-account

Read this before changing anything here. The parent repository's
[AGENTS.md](../AGENTS.md) still applies: no AI attribution in git history,
`type/topic` branches, base `develop`, SHA-pinned actions. This file adds the
rules specific to this subproject.

## What this is

Two Claude or Codex accounts live at once in Conductor, one per workspace. Three
surfaces, deliberately:

| Surface | Survives a Conductor update | Where |
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

Exempt, because they are not written by hand:

- build output under `dist/`
- `pnpm-lock.yaml`
- `LICENSE`

A file approaching the limit is a file doing more than one job. Split by
responsibility, not by line count: `lib/routes.sh` and `lib/keychain.sh`, never
`lib/conductor-acct-part-2.sh`.

### Everything in the folder that owns it

```
bin/          shell entrypoints only, thin, no logic
lib/          sourced shell libraries, one concern per file
src/panel/    TypeScript for the injected UI, plus its SCSS
dist/         build output, gitignored, never edited
tools/        patching and dev-app tooling (Python and shell)
test/         tests, split by area
docs/         prose
commands/     the /account slash command
```

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

Addresses are masked in every UI surface. If you add a surface that shows one,
mask it there too, and add it to the test.

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
test/run.sh
shellcheck -x --source-path=SCRIPTDIR bin/* lib/*.sh test/*.sh install.sh tools/*.sh
pnpm typecheck && pnpm build
```

Zero shellcheck findings is the bar; it exits non-zero on info notes too.

Then: add a `CHANGELOG.md` entry, and keep `CONDUCTOR_ACCT_VERSION` in step with
the version in the panel source. They ship together, so a skew is a bug. A test
asserts both.

## Current debt, stated rather than hidden

The 300-line rule and the folder layout above are the target. As of 0.2.0 three
files break the rule and the TypeScript build does not exist yet:

| File | Lines | Plan |
|---|---|---|
| `bin/conductor-acct` | 1360 | split into `lib/*.sh`, `bin/` keeps dispatch only |
| `tools/ui-patch/account-ui.js` | 1294 | become `src/panel/*.ts` + `styles.scss`, bundled to `dist/` |
| `test/run.sh` | 834 | split into `test/*.test.sh` with a shared harness |

Do not add to these three files without splitting them. A test enforces both
halves of that: nothing new may exceed the limit, and these three may not grow
past the length recorded in `KNOWN_LONG`. Everything else in the tree already
complies, and new code complies from the first commit.

## Escalate, don't improvise

- Destructive git operations on pushed branches, and anything touching secrets:
  ask first.
- `patch-ui.py` on `/Applications/Conductor.app`: refuse. There is a `--i-know`
  flag; it is not for agents to pass.
- Deleting a profile is the one irreversible operation here. The panel signs out
  and nothing more; `remove` stays a terminal command.
