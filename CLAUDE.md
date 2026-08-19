# CLAUDE.md

@AGENTS.md

The briefing for this subproject lives in [AGENTS.md](./AGENTS.md) (imported
above). The short version, and none of it is optional:

- **No file over 300 lines.** 301 is a failure. Build output, the lockfile, the
  licence and Markdown are the only exemptions.
- **Everything in the folder that owns it.** `bin/` entrypoints, `lib/` shell
  libraries, `src/panel/` TypeScript with `src/panel/styles/` SCSS partials,
  `dist/` build output (committed, never hand-edited), `tools/` patching,
  `test/`, `docs/`, `commands/`.
- **Build before patching.** `pnpm install && pnpm build`, then commit
  `dist/account-ui.js` if it changed. CI fails if it does not match the source.
- **No personal information.** This is published. Example addresses use
  RFC 2606 reserved domains; paths use `~` or `/Users/you`. A test enforces it.
- **`conductor-acct` owns all state.** The panel and the chat command read and
  write through it, never around it.
- **The router runs on every agent spawn**: POSIX shell, no runtime, fails open.
- **The panel must never throw.** It is injected into a compiled bundle, so an
  exception is somebody's white screen.
- **pnpm only**, `minimumReleaseAge: 10080`. No Rust: matching the host's
  language buys no extra access, as `docs/patching-conductor.md` shows.

The parent repository's [AGENTS.md](../AGENTS.md) also applies: no AI attribution
anywhere in git history, `type/topic` branches, base `develop`, SHA-pinned
actions.

Every source file is under 300 lines and a test enforces it, with no allowlist.
The next file over the limit fails the suite.
