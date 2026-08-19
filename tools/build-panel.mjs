/**
 * Builds src/panel into dist/account-ui.js, the single script `hats patch`
 * injects into Conductor.
 *
 *   node tools/build-panel.mjs           build
 *   node tools/build-panel.mjs --check   fail unless the build is reproducible
 *   node tools/build-panel.mjs --watch   rebuild on change
 *
 * One self-contained script, because the artifact is appended to a compiled
 * bundle with no module loader. Not minified: there is around 194 KB of headroom
 * in the asset slot, and readable output can be inspected in the app's devtools.
 */

import { writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import * as esbuild from "esbuild";
import * as sass from "sass";

const ROOT = path.resolve(import.meta.dirname, "..");
const ENTRY = path.join(ROOT, "src/panel/index.ts");
const OUT = path.join(ROOT, "dist/account-ui.js");

/** Resolves `import css from "./styles.scss"` to the compiled CSS as a string. */
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
    js: `/* conductor-hats: the account panel injected into Conductor's
 * frontend by \`hats patch\`.
 *
 * GENERATED FILE. Do not edit. Source is src/panel/, styles are
 * src/panel/styles.scss, build with \`pnpm build\`.
 */`,
  },
};

/**
 * Rewrites absolute module paths to repo-relative.
 *
 * esbuild labels each bundled module with its path, and for a plugin namespace
 * that path is absolute, which would put the build machine's home directory into
 * the artifact.
 */
function relativise(text) {
  return text.split(ROOT + "/").join("");
}

const args = new Set(process.argv.slice(2));

if (args.has("--watch")) {
  const ctx = await esbuild.context({ ...options, outfile: OUT });
  await ctx.watch();
  console.log("watching src/panel");
} else if (args.has("--check")) {
  /**
   * Two builds of the same source must produce identical bytes, and none may
   * name a build machine. A release attaches this artifact, so anyone should be
   * able to rebuild what was published.
   */
  const first = relativise((await esbuild.build({ ...options, write: false })).outputFiles[0].text);
  const second = relativise((await esbuild.build({ ...options, write: false })).outputFiles[0].text);
  if (first !== second) {
    console.error("the build is not reproducible: two runs differ");
    process.exit(1);
  }
  if (/\/Users\/|\/home\//.test(first)) {
    console.error("the build embeds an absolute path, so it is machine specific");
    process.exit(1);
  }
  console.log(`reproducible, ${first.length.toLocaleString()} bytes, no absolute paths`);
} else {
  await mkdir(path.dirname(OUT), { recursive: true });
  const built = await esbuild.build({ ...options, write: false });
  const text = relativise(built.outputFiles[0].text);
  await writeFile(OUT, text);
  console.log(`dist/account-ui.js  ${text.length.toLocaleString()} bytes`);
}
