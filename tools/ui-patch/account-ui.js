/* conductor-multi-account: account UI injected into Conductor's frontend.
 *
 * Appended to Conductor's main bundle by tools/patch-ui.py. It adds:
 *
 *   - a button in the workspace toolbar, next to "Open in", opening a panel to
 *     switch accounts, sign in, sign out and turn routing on or off
 *   - an account row in the New Workspace composer, so a workspace starts on
 *     the account you meant
 *
 * The panel is two levels deep, on purpose. Level one lists providers only;
 * choosing one opens its accounts, each with its own sign-out control, and a
 * single "Add new account" at the foot. A flat tree of provider groups looked
 * tidy with two accounts and would not with ten.
 *
 * The panel never deletes anything. Signing out drops that account's
 * credentials and leaves its profile, routes, session pins and transcripts
 * alone. Deleting a profile outright is `conductor-acct remove` in a terminal,
 * because it is the one irreversible operation here and a popover you can open
 * by accident is the wrong place for it.
 *
 * Everything is done against the DOM rather than by editing Conductor's React
 * code. Minified component names change on every release; "the element next to
 * the one whose tooltip says Open in" mostly does not. When an anchor does move
 * the panel simply fails to appear, which is a great deal better than a white
 * screen.
 *
 * State lives entirely in conductor-acct. This file shells out to it through
 * Conductor's own execute_shell_command, so there is one source of truth and
 * the CLI, the /account command and this panel cannot disagree.
 */
(function () {
  "use strict";

  if (window.__conductorMultiAccount) return;
  window.__conductorMultiAccount = { version: "0.2.0" };

  var CLI = "$HOME/.conductor-accounts/bin/conductor-acct";
  var log = function () {
    if (window.__conductorMultiAccountDebug)
      console.log.apply(console, ["[multi-account]"].concat([].slice.call(arguments)));
  };

  /* ---------------------------------------------------------------- shell -- */

  function sh(command) {
    var internals = window.__TAURI_INTERNALS__;
    if (!internals || !internals.invoke) return Promise.reject(new Error("no Tauri bridge"));
    return internals
      .invoke("execute_shell_command", { shell: "/bin/zsh", command: command, noRcs: true })
      .then(function (r) {
        if (r && r.code !== 0) throw new Error((r.stderr || "").trim() || "exit " + r.code);
        return ((r && r.stdout) || "").trim();
      });
  }

  function acct(args) {
    return sh(CLI + " " + args);
  }

  function q(s) {
    return "'" + String(s).replace(/'/g, "'\\''") + "'";
  }

  /* Conductor's webview runs an in-memory router, so location never changes and
   * there is no id to read. The panel works out where it is by matching what is
   * on screen against the workspaces and repositories Conductor knows about,
   * longest name first so "belo-horizonte" is not beaten by a repo called
   * "belo". Cached, because it is two SQLite reads, but not forever: a workspace
   * created after the app started would otherwise never be recognised. */
  var PLACES_TTL = 30000;
  var placesCache = null;
  var placesAt = 0;

  function places() {
    if (placesCache && Date.now() - placesAt < PLACES_TTL) {
      return Promise.resolve(placesCache);
    }
    return Promise.all([
      acct("workspaces").catch(function () { return ""; }),
      acct("repos").catch(function () { return ""; })
    ]).then(function (out) {
      function parse(text, kind) {
        return text
          .split("\n")
          .map(function (l) { return l.split("\t"); })
          .filter(function (p) { return p.length === 2 && p[0] && p[1]; })
          .map(function (p) { return { kind: kind, name: p[0], path: p[1] }; });
      }
      placesCache = parse(out[0], "workspace").concat(parse(out[1], "repository"));
      placesCache.sort(function (a, b) { return b.name.length - a.name.length; });
      placesAt = Date.now();
      return placesCache;
    });
  }

  /* Scoped to the app chrome: the sidebar lists every workspace by name, so
   * searching the whole document would match the wrong one constantly. */
  function chromeText() {
    var bits = [document.title || ""];
    var sel = "header,[class*=titlebar],[class*=toolbar],[data-tauri-drag-region]";
    var nodes = document.querySelectorAll(sel);
    for (var i = 0; i < nodes.length && i < 12; i++) bits.push(nodes[i].textContent || "");
    var btn = document.getElementById("cma-toolbar-btn");
    for (var el = btn; el && el !== document.body; el = el.parentElement) {
      bits.push(el.textContent || "");
      if ((el.textContent || "").length > 400) break;
    }
    var chip = document.getElementById("cma-chip");
    for (var e2 = chip; e2 && e2 !== document.body; e2 = e2.parentElement) {
      bits.push(e2.textContent || "");
      if ((e2.textContent || "").length > 400) break;
    }
    return bits.join(" \n ");
  }

  function currentTarget() {
    return places().then(function (list) {
      var hay = chromeText();
      for (var i = 0; i < list.length; i++) {
        if (hay.indexOf(list[i].name) >= 0) return list[i];
      }
      return { kind: "none", name: "", path: "" };
    });
  }

  /* Every read costs a shell out to conductor-acct, and `json` runs the router
   * twice inside itself to answer. Conductor re-renders constantly, so an
   * uncached read per render pass meant several process spawns a second, and a
   * click's own read then queued behind that backlog: the panel appeared late
   * enough to look like the click had been ignored.
   *
   * So: one in-flight read shared by every caller, and a short cache after it.
   * Anything that writes calls invalidate(), so the cache can never be the
   * reason a change fails to show. */
  var STATE_TTL = 4000;
  var stateCache = null;
  var stateAt = 0;
  var statePending = null;

  function invalidate() {
    stateCache = null;
    stateAt = 0;
  }

  function loadState(fresh) {
    if (!fresh && stateCache && Date.now() - stateAt < STATE_TTL) {
      return Promise.resolve(stateCache);
    }
    if (statePending) return statePending;
    statePending = currentTarget()
      .then(function (target) {
        return acct("json " + (target.path ? q(target.path) : "")).then(function (out) {
          var st = JSON.parse(out);
          st.target = target;
          return st;
        });
      })
      .then(
        function (st) {
          stateCache = st;
          stateAt = Date.now();
          statePending = null;
          return st;
        },
        function (e) {
          statePending = null;
          throw e;
        }
      );
    return statePending;
  }

  function applyAccount(state, agent, profile) {
    var t = state.target;
    if (t.kind === "workspace") return acct("use " + profile + " " + agent + " " + q(t.path));
    if (t.kind === "repository") return acct("bind " + profile + " " + agent + " " + q(t.path));
    return Promise.reject(new Error("no workspace or repository in view"));
  }

  /* ------------------------------------------------------------------ css -- */

  /* Conductor is built on shadcn conventions, so its theme tokens are already on
   * :root. Using them rather than fixed colours means this panel inherits the
   * app's palette, radii and light/dark handling instead of approximating them.
   *
   * Every clickable thing here carries cursor:pointer. Conductor's own controls
   * inherit the macOS arrow, but a popover injected over the top of the app has
   * to say out loud that it is clickable, because nothing else about it does. */
  var CSS = [
    ".cma-btn,.cma-chip{display:inline-flex;align-items:center;gap:6px;height:28px;",
    "padding:0 9px;border-radius:calc(var(--radius) - 2px);background:transparent;border:0;",
    "color:var(--muted-foreground);font-size:12px;font-weight:500;line-height:1;",
    "white-space:nowrap;cursor:pointer;transition:background .12s,color .12s}",
    ".cma-btn:hover,.cma-chip:hover,.cma-btn[aria-expanded=true],",
    ".cma-chip[aria-expanded=true]{background:var(--accent);color:var(--foreground)}",
    /* Hidden until its label is known, so the toolbar settles once instead of
     * reflowing from "Account" to the real name a moment later. */
    ".cma-btn[hidden],.cma-chip[hidden]{display:none}",

    /* Fixed width and a capped height, both on purpose: the panel's top left
     * corner is placed once when it opens and never moves again, so switching
     * view or account cannot shift anything under the pointer. A long account
     * list scrolls inside the panel rather than growing it. */
    ".cma-panel{position:fixed;z-index:99999;width:300px;box-sizing:border-box;padding:6px;",
    "max-height:min(70vh,560px);overflow-y:auto;overscroll-behavior:contain;",
    "border-radius:var(--radius);border:1px solid var(--border);",
    "background:var(--popover);color:var(--popover-foreground);",
    "box-shadow:0 10px 38px rgb(0 0 0/.28),0 2px 8px rgb(0 0 0/.16);",
    "font-size:13px;transform-origin:top left;animation:cma-in .11s ease-out}",
    "@keyframes cma-in{from{opacity:0;transform:scale(.97) translateY(-2px)}",
    "to{opacity:1;transform:none}}",
    ".cma-panel svg{flex:none;display:block}",

    ".cma-head{padding:6px 6px 8px;font-size:11px;font-weight:500;",
    "color:var(--muted-foreground)}",
    ".cma-note{padding:8px 6px 4px;font-size:11px;line-height:1.5;",
    "color:var(--muted-foreground)}",
    ".cma-code{display:block;margin-top:5px;padding:5px 7px;",
    "border-radius:calc(var(--radius) - 3px);background:var(--muted);",
    "color:var(--foreground);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;",
    "font-size:11px;user-select:all}",
    ".cma-sep{height:1px;margin:6px 2px;background:var(--border)}",

    /* Rows are bordered cards, following the wireframe: one tap target each,
     * clearly separate, and legible with a dozen accounts in the list. */
    ".cma-card{display:flex;align-items:center;gap:9px;width:100%;box-sizing:border-box;",
    "margin:0 0 5px;padding:8px 10px;text-align:left;",
    "border-radius:calc(var(--radius) - 2px);border:1px solid var(--border);",
    "background:transparent;color:inherit;font:inherit;cursor:pointer;outline:0;",
    "transition:background .12s,border-color .12s}",
    ".cma-card:last-child{margin-bottom:0}",
    ".cma-card:hover,.cma-card:focus-visible{background:var(--accent);",
    "border-color:var(--ring,var(--border))}",
    ".cma-card[aria-disabled=true]{opacity:.45;cursor:not-allowed}",
    ".cma-card[aria-disabled=true]:hover{background:transparent;border-color:var(--border)}",
    ".cma-ghost{opacity:.4;cursor:default;animation:cma-pulse 1.1s ease-in-out infinite}",
    "@keyframes cma-pulse{0%,100%{opacity:.4}50%{opacity:.62}}",
    ".cma-grow{flex:1;min-width:0}",
    ".cma-name{font-size:13px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;",
    "white-space:nowrap}",
    ".cma-sub{font-size:11px;line-height:1.35;margin-top:1px;color:var(--muted-foreground);",
    "overflow:hidden;text-overflow:ellipsis;white-space:nowrap}",
    ".cma-badge{font-size:11px;color:var(--muted-foreground);white-space:nowrap;",
    "max-width:96px;overflow:hidden;text-overflow:ellipsis}",
    ".cma-card[aria-checked=true] .cma-name{font-weight:600}",
    /* The slot is always in the flow and only its contents come and go, so the
     * tick moving between accounts does not reflow the row it lands in. */
    ".cma-tickslot{flex:none;width:15px;display:flex;justify-content:center;",
    "color:var(--foreground);opacity:.9}",

    /* An account row is one bordered card holding two buttons: the wide one
     * selects, the narrow one deletes. Nesting the delete button inside the
     * select button would be invalid HTML and would swallow the row's click, so
     * they are siblings inside a plain container that carries the border. The
     * delete control used to sit outside that border, in the gutter, which read
     * as unrelated to the row and put a destructive target a few pixels from
     * "switch account" with nothing between them. */
    ".cma-row2{display:flex;align-items:stretch;width:100%;box-sizing:border-box;",
    "margin:0 0 5px;border-radius:calc(var(--radius) - 2px);border:1px solid var(--border);",
    "overflow:hidden;transition:border-color .12s}",
    ".cma-row2:last-child{margin-bottom:0}",
    ".cma-row2:hover{border-color:var(--ring,var(--border))}",
    ".cma-row2 .cma-card{flex:1;min-width:0;width:auto;margin:0;border:0;border-radius:0}",
    ".cma-row2[aria-disabled=true]{opacity:.45}",

    /* A divider, and a target the full height of the row: deliberate to hit,
     * hard to hit by accident. */
    ".cma-signout{flex:none;display:inline-flex;align-items:center;justify-content:center;",
    "width:36px;align-self:stretch;border:0;border-left:1px solid var(--border);",
    "border-radius:0;background:transparent;color:var(--muted-foreground);",
    "cursor:pointer;opacity:.6;transition:opacity .12s,background .12s,color .12s}",
    ".cma-row2:hover .cma-signout{opacity:.85}",
    ".cma-signout:hover,.cma-signout:focus-visible{opacity:1;",
    "background:var(--destructive,#ff5a5a);color:#fff}",
    /* Signing in is not destructive, so it does not borrow the warning colour. */
    ".cma-signin:hover,.cma-signin:focus-visible{background:var(--accent);",
    "color:var(--foreground)}",
    ".cma-slot{margin-bottom:5px}",
    ".cma-slot:last-child{margin-bottom:0}",
    ".cma-slot .cma-row2{margin-bottom:0}",
    ".cma-slot .cma-form{margin-top:5px}",

    /* Masked by default. A recorded screen should not hand out an address, and
     * the profile name underneath already says which account this is. */
    ".cma-mask{font-variant-numeric:tabular-nums;letter-spacing:.01em}",

    ".cma-back{display:inline-flex;align-items:center;gap:6px;margin:0 0 6px;padding:4px 6px;",
    "border:0;border-radius:6px;background:transparent;color:var(--muted-foreground);",
    "font:inherit;font-size:12px;cursor:pointer;transition:background .12s,color .12s}",
    ".cma-back:hover{background:var(--accent);color:var(--foreground)}",
    ".cma-title{display:flex;align-items:center;gap:8px;padding:0 6px 9px;",
    "font-size:12px;font-weight:600}",

    ".cma-add{width:100%;box-sizing:border-box;margin-top:6px;padding:8px 10px;",
    "display:flex;align-items:center;justify-content:center;gap:7px;",
    "border-radius:calc(var(--radius) - 2px);border:1px dashed var(--border);",
    "background:transparent;color:var(--muted-foreground);font:inherit;font-size:12px;",
    "cursor:pointer;transition:background .12s,color .12s,border-color .12s}",
    ".cma-add:hover{background:var(--accent);color:var(--foreground);",
    "border-color:var(--ring,var(--border))}",

    ".cma-form{margin-top:6px;padding:8px;display:flex;flex-direction:column;gap:6px;",
    "border-radius:calc(var(--radius) - 2px);border:1px solid var(--border)}",
    ".cma-form .cma-note{padding:0}",
    ".cma-input{width:100%;height:30px;padding:0 9px;box-sizing:border-box;",
    "border-radius:calc(var(--radius) - 3px);border:1px solid var(--input-border,var(--border));",
    "background:var(--input,transparent);color:var(--foreground);font-size:12px;outline:0}",
    ".cma-input:focus{border-color:var(--popover-ring,var(--ring))}",
    ".cma-go{height:30px;border:0;border-radius:calc(var(--radius) - 3px);",
    "background:var(--foreground);color:var(--background);font-size:12px;font-weight:500;",
    "cursor:pointer}",
    ".cma-go:disabled{opacity:.5;cursor:default}",

    /* Deleting an account signs it out, removes its profile directory and drops
     * every route pointing at it. None of that is undoable, so it gets a real
     * dialog with a scrim rather than a control that arms on a first click:
     * an armed control is still one click from destruction, and one stray click
     * is exactly the failure being guarded against. */
    ".cma-scrim{position:fixed;inset:0;z-index:100000;display:flex;",
    "align-items:center;justify-content:center;padding:24px;",
    "background:rgb(0 0 0/.45);animation:cma-fade .12s ease-out}",
    "@keyframes cma-fade{from{opacity:0}to{opacity:1}}",
    ".cma-dialog{width:300px;max-width:100%;box-sizing:border-box;padding:14px;",
    "border-radius:var(--radius);border:1px solid var(--border);",
    "background:var(--popover);color:var(--popover-foreground);",
    "box-shadow:0 18px 48px rgb(0 0 0/.4);font-size:13px;",
    "animation:cma-pop .12s ease-out}",
    "@keyframes cma-pop{from{opacity:0;transform:scale(.96)}to{opacity:1;transform:none}}",
    ".cma-dialog .cma-name{white-space:normal;font-weight:600;font-size:13px}",
    ".cma-dialog .cma-sub{white-space:normal;margin-top:6px;line-height:1.5}",
    ".cma-actions{display:flex;gap:7px;margin-top:13px}",
    ".cma-act{flex:1;height:30px;border-radius:calc(var(--radius) - 3px);border:1px solid var(--border);",
    "background:transparent;color:inherit;font:inherit;font-size:12px;font-weight:500;",
    "cursor:pointer}",
    ".cma-act:hover{background:var(--accent)}",
    ".cma-act:disabled{opacity:.6;cursor:default}",
    ".cma-act-danger{border-color:transparent;background:var(--destructive,#ff5a5a);color:#fff}",
    ".cma-act-danger:hover{filter:brightness(1.08);background:var(--destructive,#ff5a5a)}"
  ].join("");

  function injectCss() {
    if (document.getElementById("cma-style")) return;
    var el = document.createElement("style");
    el.id = "cma-style";
    el.textContent = CSS;
    document.head.appendChild(el);
  }

  /* ---------------------------------------------------------------- icons -- */

  var SVG = "http://www.w3.org/2000/svg";
  var PATHS = {
    chevron: ["M6 3.5 10.5 8 6 12.5"],
    back: ["M12.5 8H4", "M7.5 4.5 4 8l3.5 3.5"],
    /* Sign out, not a bin: the control signs the account out and leaves
     * everything else where it was. A bin would promise deletion it does not
     * do. Sign in is the same arrow, pointing the other way. */
    signout: ["M9.2 3.5H4.2v9h5", "M7.4 8h6.4", "M11.6 5.9 13.8 8l-2.2 2.1"],
    signin: ["M6.8 3.5h5v9h-5", "M8.6 8H2.2", "M4.4 5.9 2.2 8l2.2 2.1"],
    tick: ["M3.5 8.6 6.4 11.5 12.5 5"],
    plus: ["M8 3.5v9", "M3.5 8h9"],
    /* Conductor's own Claude and Codex marks, lifted verbatim from its frontend
     * rather than approximated. Hand-drawn stand-ins read as the wrong glyph
     * next to the real ones in the model picker two rows away: an eight-line
     * asterisk is not the Anthropic sunburst, and a ringed dot is not the
     * OpenAI knot. Both are filled, on a 24-unit grid, and use currentColor, so
     * they inherit this panel's text colour like every other icon here.
     *
     * Re-extract after a Conductor release if either ever changes shape:
     *   tools/extract-assets.py grep 'M22.2819'   # the Codex path
     * The Claude mark is the path inside the component rendered beside a Claude
     * session; both are 0 0 24 24. */
    claude: ["m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"],
    codex: ["M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z"]
  };

  var FILLED = { claude: 1, codex: 1 };
  var GRID24 = { claude: 1, codex: 1 };

  function icon(name, size) {
    var svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("viewBox", GRID24[name] ? "0 0 24 24" : "0 0 16 16");
    svg.setAttribute("width", size || 14);
    svg.setAttribute("height", size || 14);
    if (FILLED[name]) {
      svg.setAttribute("fill", "currentColor");
    } else {
      svg.setAttribute("fill", "none");
      svg.setAttribute("stroke", "currentColor");
      svg.setAttribute("stroke-width", "1.4");
      svg.setAttribute("stroke-linecap", "round");
      svg.setAttribute("stroke-linejoin", "round");
    }
    svg.setAttribute("aria-hidden", "true");
    (PATHS[name] || []).forEach(function (d) {
      var p = document.createElementNS(SVG, "path");
      p.setAttribute("d", d);
      svg.appendChild(p);
    });
    return svg;
  }

  /* ---------------------------------------------------------------- panel -- */

  var open = null; /* { el, anchor, state, view, refresh } */

  function closePanel() {
    closeDialog();
    if (open && open.el && open.el.parentNode) open.el.parentNode.removeChild(open.el);
    if (open && open.anchor) open.anchor.setAttribute("aria-expanded", "false");
    open = null;
    document.removeEventListener("mousedown", onDocDown, true);
    document.removeEventListener("keydown", onDocKey, true);
  }

  /* A confirmation dialog is a sibling of the panel, not a descendant, so both
   * of these would otherwise treat interacting with it as clicking away and pull
   * the panel out from under it. */
  function onDocDown(e) {
    if (!open || openDialog) return;
    if (open.el.contains(e.target)) return;
    if (open.anchor && open.anchor.contains(e.target)) return;
    closePanel();
  }

  function onDocKey(e) {
    if (e.key !== "Escape" || !open || openDialog) return;
    if (open.view.level === "provider") {
      open.view = { level: "root" };
      render();
      return;
    }
    closePanel();
  }

  /* Conductor's New Workspace modal dismisses on pointer events it considers
   * outside itself, and a panel parked on document.body counts as outside:
   * choosing an account used to dismiss the modal and lose the typed prompt.
   *
   * The fix has two halves. Mounting inside the dialog puts the panel within
   * Conductor's containment check. Sealing pointer events at the panel's edge
   * covers listeners bound higher up the tree. The seal has to run on the
   * BUBBLE phase: sealing on capture stopped the event before it ever reached
   * the row that was clicked, which is exactly why nothing in the panel
   * responded and why the panel stopped opening at all. */
  var SEALED = ["mousedown", "pointerdown", "mouseup", "pointerup", "click",
                "touchstart", "touchend"];

  function seal(el) {
    SEALED.forEach(function (type) {
      el.addEventListener(type, function (e) { e.stopPropagation(); }, false);
    });
  }

  function mountFor(anchor) {
    for (var el = anchor; el && el !== document.body; el = el.parentElement) {
      var role = el.getAttribute && el.getAttribute("role");
      if (el.tagName === "DIALOG" || role === "dialog" || role === "alertdialog" ||
          (el.getAttribute && el.getAttribute("aria-modal") === "true")) {
        return el;
      }
    }
    return document.body;
  }

  /* Placed once, when the panel opens, and then left alone. Re-measuring on
   * every re-render is what made the panel jump: the provider view is a
   * different height from the root view, and clamping against the right edge
   * moved it sideways too. Now the top left corner is fixed for the life of the
   * panel, the width is fixed in CSS, and a long list scrolls inside it.
   *
   * position:fixed is relative to the nearest ancestor that establishes a
   * containing block, and Conductor animates its dialog with a transform, which
   * does exactly that. Rather than guess which ancestor wins, place the panel,
   * measure where it actually landed and correct by the difference. */
  function place(panel, anchor) {
    if (open && open.pos) {
      panel.style.top = open.pos.top + "px";
      panel.style.left = open.pos.left + "px";
      return;
    }
    var a = anchor.getBoundingClientRect();
    var h = panel.offsetHeight;
    var wantTop = a.bottom + 6;
    if (wantTop + h > window.innerHeight - 12) wantTop = Math.max(12, a.top - h - 6);
    var wantLeft = Math.max(12, Math.min(a.left, window.innerWidth - panel.offsetWidth - 12));

    panel.style.top = Math.round(wantTop) + "px";
    panel.style.left = Math.round(wantLeft) + "px";
    var got = panel.getBoundingClientRect();
    var dy = wantTop - got.top;
    var dx = wantLeft - got.left;
    var top = Math.round(wantTop + (Math.abs(dy) > 0.5 ? dy : 0));
    var left = Math.round(wantLeft + (Math.abs(dx) > 0.5 ? dx : 0));
    panel.style.top = top + "px";
    panel.style.left = left + "px";
    if (open) open.pos = { top: top, left: left };
  }

  function el(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }

  function label(text) {
    return el("div", "cma-head", text);
  }

  /* Profile names are lower case on disk, because they are typed at a CLI and
   * used as directory names. They are capitalised for display only: never feed
   * the result back to conductor-acct. */
  function cap(s) {
    s = String(s || "");
    return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
  }

  /* Addresses are masked wherever they appear in the UI, so a screen recording
   * or a shared screenshot cannot hand one out. Each part keeps its first three
   * and last three characters with ** between; the domain keeps its suffix so
   * the string still reads as an email:
   *
   *   someone.long@example.com  ->  som**ong@exa**le.com
   *   joe@mail.co.uk            ->  j**@m**.co.uk
   *
   * Enough to tell two accounts apart at a glance, and the profile name sits
   * right underneath for when it is not. The full address is never put in a
   * title attribute either, since a tooltip is just as visible on video. Use
   * `conductor-acct list` in a terminal when you need to read it. */
  function maskPart(s) {
    /* How much is revealed scales with length, so a short local part is not
     * handed over in full for want of characters to hide. Nothing shorter than
     * three characters reveals anything at all. */
    var n = s.length;
    if (n <= 2) return "**";
    if (n <= 5) return s.charAt(0) + "**";
    if (n <= 8) return s.slice(0, 2) + "**" + s.slice(-1);
    return s.slice(0, 3) + "**" + s.slice(-3);
  }

  function maskEmail(raw) {
    var s = String(raw || "");
    if (!s) return "";
    var at = s.lastIndexOf("@");
    if (at < 1) return maskPart(s);
    var local = s.slice(0, at);
    var domain = s.slice(at + 1);
    var dot = domain.indexOf(".");
    var host = dot > 0 ? domain.slice(0, dot) : domain;
    var suffix = dot > 0 ? domain.slice(dot) : "";
    return maskPart(local) + "@" + maskPart(host) + suffix;
  }

  var AGENT_LABEL = { claude: "Claude Code", codex: "Codex" };
  var AGENT_ICON = { claude: "claude", codex: "codex" };

  /* The trigger label shows Claude's account, falling back to whichever
   * provider has one, because that is the one people mean when they glance. */
  function primary(state) {
    var claude = (state.providers || []).filter(function (p) { return p.agent === "claude"; })[0];
    if (claude && claude.current) return claude.current;
    var any = (state.providers || []).filter(function (p) { return p.current; })[0];
    return any ? any.current : "";
  }

  function scopeText(state) {
    if (state.target.kind === "workspace") return "Workspace: " + state.target.name;
    if (state.target.kind === "repository") return "New workspaces in " + state.target.name;
    return "No workspace in view";
  }

  function footText(state) {
    if (state.target.kind === "workspace")
      return "Applies to the next chat here. A chat already running keeps the account it started on.";
    if (state.target.kind === "repository")
      return "Applies to workspaces created from now on.";
    return "Open a workspace to choose its account.";
  }

  function reload() {
    invalidate();
    return loadState(true).then(function (st) {
      if (!open) return;
      open.state = st;
      render();
      refreshTriggers(st);
    });
  }

  /* ------------------------------------------------------------ root view -- */

  function providerCard(provider) {
    var card = el("button", "cma-card");
    card.type = "button";
    card.appendChild(icon(AGENT_ICON[provider.agent], 13));

    var main = el("div", "cma-grow");
    main.appendChild(el("div", "cma-name", AGENT_LABEL[provider.agent] || provider.agent));
    var n = provider.accounts.length;
    main.appendChild(el("div", "cma-sub", n === 1 ? "1 Account" : n + " Accounts"));
    card.appendChild(main);

    card.appendChild(el("span", "cma-badge",
      cap(provider.current) || (provider.accounts.length ? "Not set" : "None")));
    card.appendChild(icon("chevron", 13));

    card.addEventListener("click", function () {
      open.view = { level: "provider", agent: provider.agent };
      render();
    });
    return card;
  }

  function rootView(state, host) {
    host.appendChild(label(scopeText(state)));
    state.providers.forEach(function (p) { host.appendChild(providerCard(p)); });

    host.appendChild(el("div", "cma-sep"));

    var toggle = el("button", "cma-card");
    toggle.type = "button";
    var tmain = el("div", "cma-grow");
    tmain.appendChild(el("div", "cma-name", state.enabled ? "Turn routing off" : "Turn routing on"));
    tmain.appendChild(el("div", "cma-sub",
      state.enabled ? "agents go back to one account" : "one account per workspace"));
    toggle.appendChild(tmain);
    toggle.addEventListener("click", function () {
      acct(state.enabled ? "uninstall" : "install").then(reload).catch(function (e) {
        note(host, String((e && e.message) || e));
      });
    });
    host.appendChild(toggle);

    host.appendChild(el("div", "cma-note", footText(state)));
  }

  /* -------------------------------------------------------- provider view -- */

  function accountSlot(state, provider, account) {
    var slot = el("div", "cma-slot");
    var row = el("div", "cma-row2");

    var card = el("button", "cma-card");
    card.type = "button";
    card.setAttribute("role", "menuitemradio");
    card.setAttribute("aria-checked", account.active ? "true" : "false");

    var main = el("div", "cma-grow");
    /* Three states, because signed in and address-known are not the same thing:
     * credentials can be in place before the address has been written anywhere
     * this can read it. */
    var shown = account.email ? maskEmail(account.email) : cap(account.name);
    var line = el("div", "cma-name" + (account.email ? " cma-mask" : ""), shown);
    if (account.email) line.setAttribute("aria-label", "email hidden");
    main.appendChild(line);
    main.appendChild(el("div", "cma-sub",
      account.email ? cap(account.name)
        : account.signedIn ? "Signed in" : "Not signed in"));
    card.appendChild(main);

    var tickslot = el("div", "cma-tickslot");
    if (account.active) tickslot.appendChild(icon("tick", 13));
    card.appendChild(tickslot);

    if (state.target.kind === "none") {
      row.setAttribute("aria-disabled", "true");
      card.setAttribute("aria-disabled", "true");
      card.title = "Open a workspace, or the New Workspace dialog, to pick an account";
    } else {
      card.addEventListener("click", function () {
        applyAccount(state, provider.agent, account.name).then(reload).catch(function (e) {
          note(open ? open.el : row, String((e && e.message) || e));
        });
      });
    }
    row.appendChild(card);

    /* Whichever of the two applies. A signed-out row with no way back in is a
     * dead end, and the row is the obvious place for the way back. */
    if (account.signedIn) {
      var out = el("button", "cma-signout");
      out.type = "button";
      out.title = "Sign out of " + cap(account.name);
      out.setAttribute("aria-label", "Sign out of " + cap(account.name));
      out.appendChild(icon("signout", 14));
      out.addEventListener("click", function () {
        confirmSignOut(provider, account);
      });
      row.appendChild(out);
    } else {
      var back = el("button", "cma-signout cma-signin");
      back.type = "button";
      back.title = "Sign in to " + cap(account.name);
      back.setAttribute("aria-label", "Sign in to " + cap(account.name));
      back.appendChild(icon("signin", 14));
      back.addEventListener("click", function () {
        signInForm(provider.agent, {
          host: slot,
          profile: account.name,
          state: state
        });
      });
      row.appendChild(back);
    }

    slot.appendChild(row);
    return slot;
  }

  /* Signing out drops that account's credentials and nothing else. The profile
   * stays, so do its routes, its session pins and its transcripts, and it
   * reappears here as "Not signed in", ready to sign back in.
   *
   * Deleting a profile outright is `conductor-acct remove` in a terminal, on
   * purpose: it is the one irreversible operation here, and a panel you open by
   * accident is the wrong place for it. */
  function confirmSignOut(provider, account) {
    dialog({
      title: "Sign out of " + cap(account.name) + "?",
      body: "Signs " + (account.email ? maskEmail(account.email) : cap(account.name)) +
            " out of " + (AGENT_LABEL[provider.agent] || provider.agent) +
            ". Nothing else changes: the account stays in this list, and its " +
            "routes, sessions and transcripts are untouched. Sign back in from " +
            "here whenever you like.",
      confirm: "Sign out",
      danger: true,
      onConfirm: function (done, fail) {
        acct("logout " + account.name + " " + provider.agent)
          .then(function () { done(); reload(); })
          .catch(function (e) { fail(String((e && e.message) || e)); });
      }
    });
  }

  /* Mounted in the same host as the panel, so Conductor's own dialog still counts
   * it as inside itself and does not dismiss on the click that opened it. */
  var openDialog = null;

  function closeDialog() {
    if (openDialog && openDialog.parentNode) openDialog.parentNode.removeChild(openDialog);
    openDialog = null;
  }

  function dialog(opts) {
    closeDialog();
    var scrim = el("div", "cma-scrim");
    var box = el("div", "cma-dialog");
    box.setAttribute("role", "alertdialog");
    box.setAttribute("aria-modal", "true");
    seal(scrim);

    box.appendChild(el("div", "cma-name", opts.title));
    var body = el("div", "cma-sub", opts.body);
    box.appendChild(body);

    var actions = el("div", "cma-actions");
    var no = el("button", "cma-act", "Cancel");
    no.type = "button";
    var yes = el("button", "cma-act" + (opts.danger ? " cma-act-danger" : ""), opts.confirm);
    yes.type = "button";

    function shut() {
      document.removeEventListener("keydown", onKey, true);
      closeDialog();
    }
    function onKey(e) {
      if (e.key === "Escape") { e.stopPropagation(); shut(); }
    }

    no.addEventListener("click", shut);
    yes.addEventListener("click", function () {
      no.disabled = true;
      yes.disabled = true;
      yes.textContent = "Working…";
      opts.onConfirm(shut, function (message) {
        yes.remove();
        no.disabled = false;
        no.textContent = "Close";
        body.textContent = message;
      });
    });
    /* Clicking the scrim cancels, which is the safe outcome. Clicking the dialog
     * itself must not, so the box stops the event before it gets there. */
    scrim.addEventListener("click", function (e) {
      if (e.target === scrim) shut();
    });

    actions.appendChild(no);
    actions.appendChild(yes);
    box.appendChild(actions);
    scrim.appendChild(box);
    (open ? open.el.parentNode : document.body).appendChild(scrim);
    openDialog = scrim;
    document.addEventListener("keydown", onKey, true);
    setTimeout(function () { no.focus(); }, 0);
  }

  /* Two callers, one flow. "Add new account" needs a name typed; the sign-in
   * control on a signed-out row already knows which profile it is for, so it
   * skips straight to the button.
   *
   *   opts.host     where the form goes
   *   opts.replace  node the form takes the place of, if any
   *   opts.profile  fixed profile name, or null to ask for one
   *   opts.state    current state, for the duplicate-address check
   */
  function signInForm(agent, opts) {
    if (opts.host.querySelector(".cma-form")) return;
    var fixed = opts.profile || null;
    var form = el("div", "cma-form");

    var name = null;
    if (!fixed) {
      name = document.createElement("input");
      name.className = "cma-input";
      name.placeholder = "name, for example work";
      name.spellcheck = false;
      form.appendChild(name);
    } else {
      form.appendChild(el("div", "cma-name", "Sign in to " + cap(fixed)));
    }

    var go = el("button", "cma-go", "Sign in");
    go.type = "button";

    var status = el("div", "cma-note", "Your browser opens for approval.");

    form.appendChild(go);
    form.appendChild(status);

    var codeField = null;
    function fail(msg) { status.textContent = msg; go.disabled = false; }

    /* One live token per account, so two profiles on one address take turns
     * signing each other out. Said here, where it just happened, rather than
     * left for someone to work out from an account that keeps logging out. */
    function warnIfDuplicate(profile, email) {
      if (!email || !opts.state) return null;
      var providers = opts.state.providers || [];
      var mine = providers.filter(function (p) { return p.agent === agent; })[0];
      if (!mine) return null;
      var clash = (mine.accounts || []).filter(function (a) {
        return a.name !== profile && a.email && a.email === email;
      })[0];
      return clash ? clash.name : null;
    }

    function poll(profile, tries) {
      acct("login-status " + profile + " " + agent)
        .then(function (out) {
          if (/^ok\b/.test(out)) {
            var email = out.slice(2).trim();
            var clash = warnIfDuplicate(profile, email);
            if (clash) {
              status.textContent = cap(clash) + " is already signed in as " +
                maskEmail(email) + ". One account cannot be two profiles: they " +
                "will sign each other out. Remove one with conductor-acct remove " +
                clash + ".";
              go.remove();
              if (codeField) codeField.remove();
              setTimeout(function () { reload(); }, 4000);
              return;
            }
            status.textContent = email
              ? "Signed in as " + maskEmail(email)
              : "Signed in.";
            setTimeout(reload, 600);
            return;
          }
          if (/^error/.test(out)) return fail(out.replace(/^error\s*/, "") || "sign-in failed");
          if (tries > 240) return fail("timed out waiting for the browser");
          setTimeout(function () { poll(profile, tries + 1); }, 1000);
        })
        .catch(function (e) { fail(String(e.message || e)); });
    }

    go.addEventListener("click", function () {
      var profile = fixed || (name ? name.value.trim() : "");
      if (!/^[A-Za-z0-9_-]+$/.test(profile)) return fail("Letters, digits, - and _ only.");
      go.disabled = true;
      status.textContent = "Starting sign-in…";
      acct("login-start " + profile + " " + agent)
        .then(function (url) {
          status.textContent = "Approve in your browser, then paste the code.";
          sh("open " + q(url)).catch(function () {});
          if (!codeField) {
            codeField = document.createElement("input");
            codeField.className = "cma-input";
            codeField.placeholder = "paste the code, then Enter";
            codeField.spellcheck = false;
            form.insertBefore(codeField, status);
            codeField.addEventListener("keydown", function (e) {
              if (e.key !== "Enter") return;
              var code = codeField.value.trim();
              if (!code) return;
              status.textContent = "Checking…";
              acct("login-code " + profile + " " + q(code))
                .then(function () { poll(profile, 0); })
                .catch(function (err) { fail(String(err.message || err)); });
            });
            setTimeout(function () { codeField.focus(); }, 0);
          }
          poll(profile, 0);
        })
        .catch(function (e) { fail(String(e.message || e)); });
    });

    if (name) {
      name.addEventListener("keydown", function (e) { if (e.key === "Enter") go.click(); });
      setTimeout(function () { name.focus(); }, 0);
    } else {
      setTimeout(function () { go.focus(); }, 0);
    }

    if (opts.replace && opts.replace.parentNode) {
      opts.replace.parentNode.replaceChild(form, opts.replace);
    } else {
      opts.host.appendChild(form);
    }
  }

  function providerView(state, host, agent) {
    var provider = state.providers.filter(function (p) { return p.agent === agent; })[0];
    if (!provider) {
      open.view = { level: "root" };
      return rootView(state, host);
    }

    var back = el("button", "cma-back");
    back.type = "button";
    back.appendChild(icon("back", 13));
    back.appendChild(el("span", null, "Back"));
    back.addEventListener("click", function () {
      open.view = { level: "root" };
      render();
    });
    host.appendChild(back);

    var title = el("div", "cma-title");
    title.appendChild(icon(AGENT_ICON[agent], 13));
    title.appendChild(el("span", null, AGENT_LABEL[agent] || agent));
    host.appendChild(title);

    if (!provider.accounts.length) {
      host.appendChild(el("div", "cma-note",
        "No accounts yet. Add one and it becomes selectable here."));
    }
    provider.accounts.forEach(function (a) {
      host.appendChild(accountSlot(state, provider, a));
    });

    var add = el("button", "cma-add");
    add.type = "button";
    add.appendChild(icon("plus", 12));
    add.appendChild(el("span", null, "Add new account"));
    add.addEventListener("click", function () {
      signInForm(agent, { host: host, replace: add, profile: null, state: state });
    });
    host.appendChild(add);

    host.appendChild(el("div", "cma-note", footText(state)));
  }

  function note(host, message) {
    var n = el("div", "cma-note", message);
    host.appendChild(n);
    setTimeout(function () { if (n.parentNode) n.parentNode.removeChild(n); }, 4000);
  }

  /* --------------------------------------------------------------- render -- */

  /* Shown while the first read is in flight, with a placeholder row per provider
   * so the panel opens at roughly its real height. It has to be roughly right:
   * the corner is pinned on this first measurement, and a panel that opened two
   * rows tall would decide it fits below the anchor and then grow off screen. */
  function loadingView(host) {
    host.appendChild(label("Loading accounts"));
    ["claude", "codex"].forEach(function (agent) {
      var card = el("div", "cma-card cma-ghost");
      card.appendChild(icon(AGENT_ICON[agent], 13));
      var main = el("div", "cma-grow");
      main.appendChild(el("div", "cma-name", AGENT_LABEL[agent]));
      main.appendChild(el("div", "cma-sub", "reading accounts"));
      card.appendChild(main);
      host.appendChild(card);
    });
    host.appendChild(el("div", "cma-sep"));
    host.appendChild(el("div", "cma-note",
      "conductor-acct is answering. This is quick once warmed up."));
  }

  function errorView(host, e) {
    /* A dead button teaches nobody anything. Show what went wrong, in the same
     * panel the accounts would have appeared in. */
    host.appendChild(label("Accounts unavailable"));
    var n = el("div", "cma-note", String((e && e.message) || e));
    n.appendChild(el("code", "cma-code", CLI + " json"));
    host.appendChild(n);
    log("panel failed", e);
  }

  function render() {
    if (!open) return;
    var host = open.el;
    host.replaceChildren();
    if (open.error) errorView(host, open.error);
    else if (!open.state) loadingView(host);
    else if (open.view.level === "provider") providerView(open.state, host, open.view.agent);
    else rootView(open.state, host);
    place(host, open.anchor);
  }

  function listen() {
    setTimeout(function () {
      document.addEventListener("mousedown", onDocDown, true);
      document.addEventListener("keydown", onDocKey, true);
    }, 0);
  }

  /* Opens on the first event rather than after a round trip. Waiting for the
   * state read before showing anything is what made a click look ignored, and
   * clicking again then only toggled the panel that had not appeared yet. */
  function togglePanel(anchor) {
    if (open) {
      var same = open.anchor === anchor;
      closePanel();
      if (same) return;
    }
    anchor.setAttribute("aria-expanded", "true");
    var panel = el("div", "cma-panel");
    panel.setAttribute("role", "menu");
    seal(panel);
    mountFor(anchor).appendChild(panel);
    open = { el: panel, anchor: anchor, state: null, error: null, view: { level: "root" } };
    render();
    listen();

    loadState()
      .then(function (state) {
        /* Closed, or reopened against another trigger, while this was in flight. */
        if (!open || open.el !== panel) return;
        open.state = state;
        render();
        refreshTriggers(state);
      })
      .catch(function (e) {
        if (!open || open.el !== panel) return;
        open.error = e;
        render();
      });
  }

  /* Opens on press rather than on click, for one specific reason: Conductor
   * re-renders its toolbar constantly, and when React replaces the container the
   * trigger is rebuilt with it. A rebuild landing between mousedown and mouseup
   * means the browser fires no click at all, so the press did nothing and you
   * pressed again. A single event cannot be split that way.
   *
   * This is also how native menus behave, so it feels faster besides. `click` is
   * still handled for keyboard activation, guarded so a real pointer press does
   * not toggle twice. */
  function openOnPress(trigger) {
    var pressedAt = 0;
    trigger.addEventListener("pointerdown", function (e) {
      if (e.button !== undefined && e.button !== 0) return;
      e.preventDefault();
      pressedAt = Date.now();
      togglePanel(trigger);
    });
    trigger.addEventListener("click", function (e) {
      e.preventDefault();
      if (Date.now() - pressedAt < 700) return;
      togglePanel(trigger);
    });
  }

  /* Both triggers are refreshed from state already in hand. They can each fetch
   * their own, and do on first attach, but a switch would then cost three reads
   * of the same thing and the labels would visibly lag the tick. */
  function refreshTriggers(state) {
    var b = document.getElementById("cma-toolbar-btn");
    if (b) refreshToolbarLabel(b, state);
    refreshComposerChip(state);
  }

  /* -------------------------------------------------------------- toolbar -- */

  /* The "Open in" control is identified by its accessible name rather than by a
   * class, because class names are hashed per build. */
  function findOpenIn() {
    var nodes = document.querySelectorAll("button,[role=button],a,[data-slot=button]");
    /* Accessible name first, then visible text, then a tooltip on an ancestor.
     * Conductor renders this control as an icon, so which of the three carries
     * the words "Open in" varies. */
    for (var i = 0; i < nodes.length; i++) {
      var n = nodes[i];
      var lbl = (n.getAttribute("aria-label") || n.getAttribute("title") ||
                 n.getAttribute("data-tooltip") || "").trim();
      if (/open (in|remote)/i.test(lbl)) return n;
    }
    for (var j = 0; j < nodes.length; j++) {
      var t = (nodes[j].textContent || "").trim();
      if (t.length < 24 && /open in/i.test(t)) return nodes[j];
    }
    /* Last resort, and in practice the reliable one: the control renders the
     * chosen app's icon from /app-icons/, so find that image and climb to the
     * button wrapping it. */
    var ic = document.querySelector('img[src*="app-icons"],img[src*="finder.png"]');
    while (ic && ic !== document.body) {
      if (ic.tagName === "BUTTON" || ic.getAttribute("role") === "button") return ic;
      ic = ic.parentElement;
    }
    return null;
  }

  /* When the toolbar cannot be found the button goes top right instead of
   * silently not existing. A control in slightly the wrong place is debuggable;
   * nothing at all looks identical to the script never having run. */
  function floatingHost() {
    var host = document.getElementById("cma-float");
    if (host) return host;
    host = document.createElement("div");
    host.id = "cma-float";
    host.style.cssText = "position:fixed;top:9px;right:14px;z-index:99998";
    document.body.appendChild(host);
    return host;
  }

  var missedToolbar = 0;

  function toolbarButton() {
    var existing = document.getElementById("cma-toolbar-btn");
    var anchor = findOpenIn();
    var host, before = null;

    if (anchor && anchor.parentElement) {
      host = anchor.parentElement;
      before = anchor;
      missedToolbar = 0;
    } else {
      /* Give the real toolbar a few render passes before giving up on it. */
      if (++missedToolbar < 8) return;
      host = floatingHost();
    }

    /* Left alone while it is still in the document and still in the right place.
     * Re-reading the label here ran on every render pass Conductor made, which
     * during a streaming chat is several a second, each one a process spawn. */
    if (existing && existing.isConnected && existing.parentElement === host) return;
    if (existing && existing.parentNode) existing.parentNode.removeChild(existing);

    var btn = document.createElement("button");
    btn.id = "cma-toolbar-btn";
    btn.type = "button";
    btn.className = "cma-btn";
    btn.setAttribute("aria-label", "Agent account");
    btn.hidden = true;
    btn.appendChild(el("span", "cma-label"));
    seal(btn);
    openOnPress(btn);

    if (before) host.insertBefore(btn, before);
    else host.appendChild(btn);
    refreshToolbarLabel(btn);
    log("toolbar button attached", before ? "next to Open in" : "floating (toolbar not found)");
  }

  function refreshToolbarLabel(btn, state) {
    function apply(s) {
      var cur = primary(s);
      var lbl = btn.querySelector(".cma-label");
      if (lbl) lbl.textContent = cap(cur) || (s.enabled ? "Default" : "Off");
      btn.title = cur ? "Agent account: " + cap(cur) : "No account chosen here";
      btn.hidden = false;
    }
    if (state) return apply(state);
    loadState()
      .then(apply)
      .catch(function (e) {
        var lbl = btn.querySelector(".cma-label");
        if (lbl) lbl.textContent = "Account?";
        btn.title = "conductor-acct did not answer: " + (e && e.message ? e.message : e);
        btn.hidden = false;
      });
  }

  /* ------------------------------------------------------------- composer -- */

  /* The New Workspace composer is found by its placeholder, which is product
   * copy and changes far less often than any generated identifier. */
  function findComposer() {
    var els = document.querySelectorAll("[placeholder],[data-placeholder]");
    for (var i = 0; i < els.length; i++) {
      var p = els[i].getAttribute("placeholder") || els[i].getAttribute("data-placeholder") || "";
      if (/what do you want to work on/i.test(p)) return els[i];
    }
    return null;
  }

  function composerFooter(node) {
    /* Walk up to the composer card, then take its last row: the one holding the
     * model picker and the Create button. */
    var e = node;
    for (var i = 0; i < 8 && e; i++, e = e.parentElement) {
      var rows = e.querySelectorAll(":scope > div");
      if (rows.length >= 2) {
        var last = rows[rows.length - 1];
        if (last.querySelector("button") && last.textContent.length < 400) return last;
      }
    }
    return null;
  }

  function composerChip() {
    var composer = findComposer();
    if (!composer) return;
    var foot = composerFooter(composer);
    if (!foot) return;
    /* Same as the toolbar button: left alone while it is where it belongs, so a
     * render pass does not cost a state read. */
    if (foot.querySelector("#cma-chip")) return;

    var chip = document.createElement("button");
    chip.id = "cma-chip";
    chip.type = "button";
    chip.className = "cma-chip";
    chip.hidden = true;
    chip.appendChild(el("span", "cma-label"));
    seal(chip);
    openOnPress(chip);
    foot.insertBefore(chip, foot.firstChild);
    refreshComposerChip();
    log("composer chip attached");
  }

  function refreshComposerChip(state) {
    var chip = document.getElementById("cma-chip");
    if (!chip) return;
    function apply(s) {
      var lbl = chip.querySelector(".cma-label");
      var name = cap(primary(s)) || "Default account";
      if (lbl) lbl.textContent = name;
      chip.title = "This workspace will run agents on: " + name;
      chip.hidden = false;
    }
    if (state) return apply(state);
    loadState().then(apply).catch(function () {});
  }

  /* ----------------------------------------------------------------- boot -- */

  function tick() {
    try {
      injectCss();
      toolbarButton();
      composerChip();
    } catch (e) {
      log("tick failed", e);
    }
  }

  function boot() {
    tick();
    /* Conductor re-renders constantly, so rather than fight React, re-attach on
     * a coalesced observer. Cheap: both attach paths bail immediately when the
     * elements are already in place, and neither reads any state. */
    var pending = null;
    new MutationObserver(function () {
      if (pending) return;
      pending = setTimeout(function () {
        pending = null;
        tick();
      }, 250);
    }).observe(document.body, { childList: true, subtree: true });

    /* The labels used to be refreshed by the observer, which meant a process
     * spawn per render pass. A slow timer keeps them current at a fixed, small
     * cost instead: switching from a terminal shows up within a few seconds, and
     * switching from the panel is immediate because the panel already knows. */
    setInterval(function () {
      if (!document.getElementById("cma-toolbar-btn") &&
          !document.getElementById("cma-chip")) return;
      loadState().then(refreshTriggers).catch(function () {});
    }, 8000);
    log("ready");
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    setTimeout(boot, 800);
  }
})();
