# Agent briefing: conductor-hats

Read this before changing anything here.

Git rules, carried over from where this project started and still in force: no AI
attribution anywhere in history, `type/topic` branches, and every GitHub Action
pinned to a full commit SHA fetched from the API rather than written from
memory.

## What this is

Any number of Claude Code or Codex accounts live at once in Conductor, one per
workspace. The point of the project is the panel injected into Conductor's own
toolbar; a slash command is something anyone can write, and it is the fallback
for an unpatched install rather than the feature. Three ways to drive it:

| Where you drive it | Survives a Conductor update | Code |
|---|---|---|
| `hats` CLI | yes | `bin/`, `lib/` |
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
responsibility, not by line count: `routes.rs` and `profile.rs`, never
`cli-part-2.rs`.

### Everything in the folder that owns it

```
src/rust/       the hats binary: routing, the CLI, patching, the dev app
src/panel/      TypeScript for the injected UI
src/panel/styles/  SCSS partials, one per group of elements
dist/           built panel and boot guard, generated and gitignored
target/         Rust build output, gitignored
tools/          the panel build and the version script
tests/          cargo integration tests, one file per area
docs/           prose
commands/       the /account slash command
```

One binary answers to four names. `install.sh` symlinks `hats`,
`claude-router` and `codex-router` at `hats`, and it reports itself as whichever
name invoked it.

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

`hats` owns all state. The panel and the chat command read it and write
through it; neither keeps its own copy. If the panel and the CLI can disagree
about anything, that is the bug.

The one deliberate duplication is the masking rule, because the panel cannot
shell out once per row. A test runs both implementations over the same cases and
fails if they differ. Any future duplication needs the same treatment.

### The router is on the hot path

The router runs on **every agent spawn**, so:

- **fails open.** The decision runs inside `catch_unwind`, and any failure leaves
  the environment untouched and still `exec`s the agent. A broken install costs
  the routing, never the agent. Three tests cover it: an unreadable routes file,
  a nonsense one, and no accounts root at all.
- **`exec`, never fork.** The agent's spare host and Conductor's stdio pipes both
  assume a direct child.
- `panic = "abort"` must stay out of the release profile. An abort in the router
  means no agent starts, which is the exact catastrophe fail-open exists to
  prevent.

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
- **Rust** for everything the user runs: routing, the CLI, patching, the dev app.
- **TypeScript + esbuild** for `src/panel/`, bundled to self-contained IIFEs: the
  panel and the boot guard. Each injected artifact has to be a single script with
  no module loader, so many small sources plus a build step is the only way to
  keep the 300-line rule.
- **SCSS** for panel styles, compiled and inlined into the bundle at build time.
- Stock macOS tools are shelled out to freely. codesign, security, PlistBuddy and
  xattr cost a user nothing to have; a runtime does. That is the line.
- Rust buys no extra *access* to Conductor, which `docs/patching-conductor.md`
  still sets out with evidence. It is here for distribution: one binary, no
  runtime.

Build before patching:

```sh
pnpm install
pnpm build          # src/panel + styles.scss -> dist/*.js
cargo build --release   # embeds both scripts into the binary
```

## Before opening a pull request

```sh
pnpm install
pnpm typecheck
pnpm build            # the binary embeds both scripts
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
shellcheck -x --source-path=SCRIPTDIR install.sh
```

Zero findings is the bar in each. install.sh is the only shell in the
repository: it is the `curl | sh` bootstrap, so it runs before there is a binary
to run anything else with.

Then: add a `CHANGELOG.md` entry, and keep `CONDUCTOR_ACCT_VERSION` in step with
the version in the panel source. They ship together, so a skew is a bug. A test
asserts both.

## No current debt

Every source file is under 300 lines and in the folder that owns it, and a test
enforces both. There is no allowlist to add to: the next file over the limit fails
the suite.

Files that outgrew the limit were split rather than exempted: the injected script
into `src/panel/*.ts` plus SCSS partials, the test suite into `test/harness.sh`
and one file per area, and the Rust dispatch into `cli.rs` when `main.rs` reached
349 lines.

## Escalate, don't improvise

- Destructive git operations on pushed branches, and anything touching secrets:
  ask first.
- `hats patch` on `/Applications/Conductor.app`: refuse. There is a `--i-know`
  flag; it is not for agents to pass.
- Deleting a profile is the one irreversible operation here. The panel signs out
  and nothing more; `remove` stays a terminal command.
