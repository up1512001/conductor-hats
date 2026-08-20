# CLAUDE.md

@AGENTS.md

The briefing for this subproject lives in [AGENTS.md](./AGENTS.md) (imported
above). The short version, and none of it is optional:

- **No file over 300 lines.** 301 is a failure. Build output, the lockfile, the
  licence and Markdown are the only exemptions.
- **Everything in the folder that owns it.** `src/rust/` the binary, `src/panel/`
  TypeScript with `src/panel/styles/` SCSS partials, `dist/` and `target/`
  generated, `tools/`, `test/`, `docs/`, `commands/`.
- **Build before patching.** `pnpm install && pnpm build`, then
  `hats patch`. `dist/` is generated, never committed.
- **No personal information.** This is published. Example addresses use
  RFC 2606 reserved domains; paths use `~` or `/Users/you`. A test enforces it.
- **`hats` owns all state.** The panel and the chat command read and
  write through it, never around it.
- **The router runs on every agent spawn**: fails open through `catch_unwind`, and
  `panic = "abort"` must stay out of the release profile.
- **The panel must never throw.** It is injected into a compiled bundle, so an
  exception is somebody's white screen.
- **pnpm 11 only**, settings in `pnpm-workspace.yaml`, `minimumReleaseAge: 10080`.
- **Rust for the `hats` binary**, which carries the panel and needs no runtime.
  Stock macOS tools may be shelled out to; a Python or Node runtime may not.
- **Docblocks only, and no `//` comments in any file.** `/** */` in TypeScript,
  JavaScript and SCSS; `//!` and `///` in Rust; `#` at column zero in shell, never
  indented inside a body. Directives (`# shellcheck`, `cargo:`, `#!`) are exempt.
  A test enforces it.

Git rules: no AI attribution anywhere in history, `type/topic` branches, and
every GitHub Action pinned to a commit SHA fetched from the API.

Every source file is under 300 lines and a test enforces it, with no allowlist.
The next file over the limit fails the suite.
