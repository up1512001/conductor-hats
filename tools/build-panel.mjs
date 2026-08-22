/**
 * Builds src/panel into the two scripts `hats patch` injects into Conductor:
 * dist/account-ui.js, the panel, and dist/boot-guard.js, which has to run
 * before Conductor's own modules.
 *
 *   node tools/build-panel.mjs           build
 *   node tools/build-panel.mjs --check   fail unless the build is reproducible
 *   node tools/build-panel.mjs --watch   rebuild on change
 *
 * Self-contained scripts, because each is spliced into a compiled bundle with no
 * module loader. Not minified: there is headroom in both asset slots, and
 * readable output can be inspected in the app's devtools.
 */

import { writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import * as esbuild from "esbuild";
import * as sass from "sass";

const ROOT = path.resolve(import.meta.dirname, "..");
const TARGETS = [
  { entry: "src/panel/index.ts", out: "dist/account-ui.js", what: "the account panel" },
  { entry: "src/panel/guard.ts", out: "dist/boot-guard.js", what: "the boot guard" },
];

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

const shared = {
  bundle: true,
  format: "iife",
  target: "es2020",
  platform: "browser",
  legalComments: "inline",
  charset: "utf8",
  plugins: [scssPlugin],
};

function optionsFor(target) {
  return {
    ...shared,
    entryPoints: [path.join(ROOT, target.entry)],
    banner: {
      js: `/* conductor-hats: ${target.what}, spliced into Conductor's frontend
 * by \`hats patch\`.
 *
 * GENERATED FILE. Do not edit. Source is ${target.entry}, build with
 * \`pnpm build\`.
 */`,
    },
  };
}

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
  for (const target of TARGETS) {
    const ctx = await esbuild.context({
      ...optionsFor(target),
      outfile: path.join(ROOT, target.out),
    });
    await ctx.watch();
  }
  console.log("watching src/panel");
} else if (args.has("--check")) {
  /**
   * Two builds of the same source must produce identical bytes, and none may
   * name a build machine. A release attaches these artifacts, so anyone should
   * be able to rebuild what was published.
   */
  for (const target of TARGETS) {
    const options = optionsFor(target);
    const first = relativise((await esbuild.build({ ...options, write: false })).outputFiles[0].text);
    const second = relativise((await esbuild.build({ ...options, write: false })).outputFiles[0].text);
    if (first !== second) {
      console.error(`${target.out} is not reproducible: two runs differ`);
      process.exit(1);
    }
    if (/\/Users\/|\/home\//.test(first)) {
      console.error(`${target.out} embeds an absolute path, so it is machine specific`);
      process.exit(1);
    }
    console.log(`${target.out}  reproducible, ${first.length.toLocaleString()} bytes, no absolute paths`);
  }
} else {
  for (const target of TARGETS) {
    const out = path.join(ROOT, target.out);
    await mkdir(path.dirname(out), { recursive: true });
    const built = await esbuild.build({ ...optionsFor(target), write: false });
    const text = relativise(built.outputFiles[0].text);
    await writeFile(out, text);
    console.log(`${target.out}  ${text.length.toLocaleString()} bytes`);
  }
}
