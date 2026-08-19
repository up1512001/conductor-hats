// Build src/panel into the single script that gets injected into Conductor.
//
//   node tools/build-panel.mjs           build dist/account-ui.js
//   node tools/build-panel.mjs --check   build to memory, fail if dist differs
//   node tools/build-panel.mjs --watch   rebuild on change
//
// Why a bundler at all: the artifact is appended to Conductor's compiled
// frontend, where there is no module loader and no second file to load. It has
// to be one self-contained script. Bundling is what lets the source be many
// small files instead of one unreadable one.
//
// Why not minified: there is ~194 KB of headroom in the asset slot, so minifying
// buys nothing that matters, and unminified output can be read in the app's
// devtools when an anchor breaks after a Conductor release. Legibility is worth
// more here than bytes.

import { readFile, writeFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import * as esbuild from "esbuild";
import * as sass from "sass";

const ROOT = path.resolve(import.meta.dirname, "..");
const ENTRY = path.join(ROOT, "src/panel/index.ts");
const OUT = path.join(ROOT, "dist/account-ui.js");

// Resolves `import css from "./styles.scss"` to a module exporting the compiled
// CSS as a string, so styles live in a real .scss file rather than a JavaScript
// array of string fragments.
const scssPlugin = {
  name: "scss-text",
  setup(build) {
    build.onResolve({ filter: /\.scss$/ }, (args) => ({
      path: path.resolve(args.resolveDir, args.path),
      namespace: "scss-text",
    }));
    build.onLoad({ filter: /.*/, namespace: "scss-text" }, async (args) => {
      const result = await sass.compileAsync(args.path, {
        style: "compressed",
        loadPaths: [path.dirname(args.path)],
      });
      return {
        contents: `export default ${JSON.stringify(result.css)};`,
        loader: "js",
        watchFiles: [args.path, ...result.loadedUrls.map((u) => u.pathname)],
      };
    });
  },
};

const options = {
  entryPoints: [ENTRY],
  bundle: true,
  format: "iife",
  target: "es2020",
  platform: "browser",
  legalComments: "inline",
  charset: "utf8",
  plugins: [scssPlugin],
  banner: {
    js: `/* conductor-multi-account: the account panel injected into Conductor's
 * frontend by tools/patch-ui.py.
 *
 * GENERATED FILE. Do not edit. Source is src/panel/, styles are
 * src/panel/styles.scss, build with \`pnpm build\`.
 */`,
  },
};

const args = new Set(process.argv.slice(2));

if (args.has("--watch")) {
  const ctx = await esbuild.context({ ...options, outfile: OUT });
  await ctx.watch();
  console.log("watching src/panel");
} else if (args.has("--check")) {
  const built = await esbuild.build({ ...options, write: false });
  const fresh = built.outputFiles[0].text;
  if (!existsSync(OUT)) {
    console.error(`missing ${path.relative(ROOT, OUT)}: run 'pnpm build' and commit it`);
    process.exit(1);
  }
  const committed = await readFile(OUT, "utf8");
  if (committed !== fresh) {
    console.error(
      `${path.relative(ROOT, OUT)} is stale.\n` +
        "It is committed so that patching needs no toolchain, which only works\n" +
        "if it matches the source. Run 'pnpm build' and commit the result."
    );
    process.exit(1);
  }
  console.log(`${path.relative(ROOT, OUT)} matches src/panel (${fresh.length} bytes)`);
} else {
  await mkdir(path.dirname(OUT), { recursive: true });
  const built = await esbuild.build({ ...options, outfile: OUT, metafile: true });
  const bytes = Object.values(built.metafile.outputs)[0].bytes;
  console.log(`dist/account-ui.js  ${bytes.toLocaleString()} bytes`);
}
