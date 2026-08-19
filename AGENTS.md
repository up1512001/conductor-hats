# Agent briefing: conductor-hats

Read this before changing anything here. The parent repository's
[AGENTS.md](../AGENTS.md) still applies: no AI attribution in git history,
`type/topic` branches, base `develop`, SHA-pinned actions. This file adds the
rules specific to this subproject.

## What this is

Any number of Claude Code or Codex accounts live at once in Conductor, one per
workspace. The point of the project is the panel injected into Conductor's own
toolbar; a slash command is something anyone can write, and it is the fallback
for an unpatched install rather than the feature. Three ways to drive it:

| Where you drive it | Survives a Conductor update | Code |
|---|---|---|
| `conductor-acct` CLI | yes | `bin/`, `lib/` |
| `/account` in the chat | yes | `commands/account.md` |
| Injected in-app panel | **no**, re-apply after each release | `src/panel/`, `tools/` |

`docs/how-it-works.md` is the routing mechanism, `docs/account-panel.md` is what
the panel does, `docs/panel-internals.md` is how it attaches, and
`docs/patching-conductor.md` is what the app bundle does and does not allow.

## Hard rules

### Docblocks only. No `//` comments anywhere

One comment form per language, at the top of a file or a declaration:

| Language | Allowed | Never |
|---|---|---|
| TypeScript, JavaScript, SCSS | `/** ... */` | `//` anything |
| Rust | `//!` for a module, `///` for an item | `//` anything |
| Shell | `#` at column zero | `#` indented inside a body |

If a fact is load-bearing it belongs in the docblock of the thing it describes.
If it narrates what the next line does, delete it. A comment inside a function
body is the tell.

Directives are not comments and are exempt: `# shellcheck disable=`, `cargo:`,
`#!` shebangs.

A test enforces all of this. It was asked for twice and drifted back both times,
so it is no longer a matter of remembering.

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
dist/           build output, generated and gitignored
tools/          patching and dev-app tooling (Python and shell)
test/           harness.sh plus one *.test.sh per area
docs/           prose
commands/       the /account slash command
```

`dist/` is generated and never committed. Build output in git goes stale, muddies
diffs, and in this case leaked a real home directory: esbuild labels each bundled
module with its path, and for the SCSS plugin that path was absolute. The build
rewrites those to repo-relative now, and `pnpm verify` fails if two builds differ
or if any absolute path survives, because a release attaches this artifact and
anyone should be able to rebuild exactly what was published.

So patching needs a build first: `pnpm install && pnpm build`, then
`hats patch`. For anyone who only wants the panel and no toolchain, the
built file belongs in a GitHub Release, not in the tree.

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

- **pnpm 11 only**, never npm or yarn. Settings live in `pnpm-workspace.yaml`,
  which is where pnpm 11 reads them; it ignores both the `pnpm` field in
  `package.json` and `.npmrc`. `minimumReleaseAge: 10080` holds every dependency
  back a week, and `allowBuilds` answers which packages may run install scripts so
  an install never waits on a prompt.
- **Shell** for `bin/` and `lib/`: the hot path, per above.
- **TypeScript + esbuild** for `src/panel/`, bundled to one self-contained IIFE.
  The injected artifact has to be a single script with no module loader, so many
  small sources plus a build step is the only way to keep the 300-line rule.
- **SCSS** for panel styles, compiled and inlined into the bundle at build time.
- **Rust** for the `hats` binary: Mach-O parsing, brotli, building the isolated
  copy, and injecting the panel. It carries the compiled panel inside it, so a
  user needs no Python, Node or brotli command.
- Stock macOS tools are shelled out to freely. codesign, security, PlistBuddy and
  xattr cost a user nothing to have; a runtime does. That is the line.
- Rust buys no extra *access* to Conductor, which `docs/patching-conductor.md`
  still sets out with evidence. It is here for distribution: one binary, no
  runtime.

Build before patching:

```sh
pnpm install
pnpm build          # src/panel + styles.scss -> dist/account-ui.js
cargo build --release   # embeds dist/account-ui.js into the binary
```

## Before opening a pull request

```sh
pnpm install
pnpm typecheck
pnpm build            # tests that read the artifact need it built
test/run.sh
shellcheck -x --source-path=SCRIPTDIR \
  bin/conductor-acct bin/_resolve.sh bin/claude-router bin/codex-router \
  lib/*.sh test/run.sh test/harness.sh test/*.test.sh \
  install.sh
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
- `hats patch` on `/Applications/Conductor.app`: refuse. There is a `--i-know`
  flag; it is not for agents to pass.
- Deleting a profile is the one irreversible operation here. The panel signs out
  and nothing more; `remove` stays a terminal command.
