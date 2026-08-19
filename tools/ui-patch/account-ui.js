/* conductor-multi-account: account UI injected into Conductor's frontend.
 *
 * Appended to Conductor's main bundle by tools/patch-ui.py. It adds:
 *
 *   - a button in the workspace toolbar, next to "Open in", opening a panel to
 *     switch, add, remove and enable/disable accounts
 *   - an account row in the New Workspace composer, so a workspace starts on
 *     the account you meant
 *
 * The panel is two levels deep, on purpose. Level one lists providers only;
 * choosing one opens its accounts, each with its own delete control, and a
 * single "Add new account" at the foot. A flat tree of provider groups looked
 * tidy with two accounts and would not with ten.
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
   * "belo". Cached, because it is two SQLite reads. */
  var placesCache = null;

  function places() {
    if (placesCache) return Promise.resolve(placesCache);
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

  function loadState() {
    return currentTarget().then(function (target) {
      return acct("json " + (target.path ? q(target.path) : "")).then(function (out) {
        var st = JSON.parse(out);
        st.target = target;
        return st;
      });
    });
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
    ".cma-dot{width:6px;height:6px;border-radius:50%;background:currentColor;flex:none;opacity:.75}",
    ".cma-dot.cma-off{opacity:.35}",

    ".cma-panel{position:fixed;z-index:99999;width:300px;padding:6px;",
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
    ".cma-grow{flex:1;min-width:0}",
    ".cma-name{font-size:13px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;",
    "white-space:nowrap}",
    ".cma-sub{font-size:11px;line-height:1.35;margin-top:1px;color:var(--muted-foreground);",
    "overflow:hidden;text-overflow:ellipsis;white-space:nowrap}",
    ".cma-badge{font-size:11px;color:var(--muted-foreground);white-space:nowrap;",
    "max-width:96px;overflow:hidden;text-overflow:ellipsis}",
    ".cma-card[aria-checked=true] .cma-name{font-weight:600}",
    ".cma-tick{color:var(--foreground);opacity:.9}",

    /* The delete control sits inside a row that is itself a button, so it is a
     * sibling rather than a nested button: nested interactive elements are
     * invalid and, worse, swallow the row's own click. */
    ".cma-trash{flex:none;display:inline-flex;align-items:center;justify-content:center;",
    "width:26px;height:26px;margin:-4px -5px -4px 0;border:0;border-radius:6px;",
    "background:transparent;color:var(--muted-foreground);cursor:pointer;opacity:.55;",
    "transition:opacity .12s,background .12s,color .12s}",
    ".cma-rowwrap{position:relative;display:flex;align-items:center;gap:0}",
    ".cma-rowwrap .cma-card{margin-bottom:0}",
    ".cma-rowwrap:hover .cma-trash{opacity:.85}",
    ".cma-trash:hover{opacity:1;background:var(--destructive,#ff5a5a);color:#fff}",
    ".cma-slot{margin-bottom:5px}",
    ".cma-slot:last-child{margin-bottom:0}",

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

    ".cma-confirm{padding:8px 10px;border-radius:calc(var(--radius) - 2px);",
    "border:1px solid var(--destructive,#ff5a5a)}",
    ".cma-confirm .cma-sub{white-space:normal}",
    ".cma-actions{display:flex;gap:6px;margin-top:7px}",
    ".cma-act{flex:1;height:28px;border-radius:calc(var(--radius) - 3px);border:1px solid var(--border);",
    "background:transparent;color:inherit;font:inherit;font-size:12px;cursor:pointer}",
    ".cma-act:hover{background:var(--accent)}",
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
    trash: ["M3 5h10", "M6.5 5V3.5h3V5", "M4.5 5.5 5 13h6l.5-7.5", "M7 7.5v3.5", "M9 7.5v3.5"],
    tick: ["M3.5 8.6 6.4 11.5 12.5 5"],
    plus: ["M8 3.5v9", "M3.5 8h9"],
    claude: ["M8 2.2v11.6", "M2.2 8h11.6", "M3.9 3.9l8.2 8.2", "M12.1 3.9l-8.2 8.2"],
    codex: ["M8 2.6a5.4 5.4 0 1 0 0 10.8A5.4 5.4 0 0 0 8 2.6z", "M8 6.4a1.6 1.6 0 1 0 0 3.2 1.6 1.6 0 0 0 0-3.2z"]
  };

  function icon(name, size) {
    var svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("viewBox", "0 0 16 16");
    svg.setAttribute("width", size || 14);
    svg.setAttribute("height", size || 14);
    svg.setAttribute("fill", "none");
    svg.setAttribute("stroke", "currentColor");
    svg.setAttribute("stroke-width", "1.4");
    svg.setAttribute("stroke-linecap", "round");
    svg.setAttribute("stroke-linejoin", "round");
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
    if (open && open.el && open.el.parentNode) open.el.parentNode.removeChild(open.el);
    if (open && open.anchor) open.anchor.setAttribute("aria-expanded", "false");
    open = null;
    document.removeEventListener("mousedown", onDocDown, true);
    document.removeEventListener("keydown", onDocKey, true);
  }

  function onDocDown(e) {
    if (!open) return;
    if (open.el.contains(e.target)) return;
    if (open.anchor && open.anchor.contains(e.target)) return;
    closePanel();
  }

  function onDocKey(e) {
    if (e.key !== "Escape" || !open) return;
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

  /* position:fixed is relative to the nearest ancestor that establishes a
   * containing block, and Conductor animates its dialog with a transform, which
   * does exactly that. Rather than guess which ancestor wins, place the panel,
   * measure where it actually landed and correct by the difference. */
  function place(panel, anchor) {
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
    if (Math.abs(dy) > 0.5 || Math.abs(dx) > 0.5) {
      panel.style.top = Math.round(wantTop + dy) + "px";
      panel.style.left = Math.round(wantLeft + dx) + "px";
    }
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
    return loadState().then(function (st) {
      if (!open) return;
      open.state = st;
      render();
      refreshTriggers();
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
    main.appendChild(el("div", "cma-sub", n === 1 ? "1 account" : n + " accounts"));
    card.appendChild(main);

    card.appendChild(el("span", "cma-badge",
      provider.current || (provider.accounts.length ? "not set" : "none")));
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
    var wrap = el("div", "cma-rowwrap");

    var card = el("button", "cma-card");
    card.type = "button";
    card.setAttribute("role", "menuitemradio");
    card.setAttribute("aria-checked", account.active ? "true" : "false");

    var main = el("div", "cma-grow");
    main.appendChild(el("div", "cma-name", account.email || account.name));
    main.appendChild(el("div", "cma-sub", account.email ? account.name : "not signed in"));
    card.appendChild(main);
    if (account.active) {
      var t = icon("tick", 13);
      t.setAttribute("class", "cma-tick");
      card.appendChild(t);
    }

    if (state.target.kind === "none") {
      card.setAttribute("aria-disabled", "true");
      card.title = "Open a workspace, or the New Workspace dialog, to pick an account";
    } else {
      card.addEventListener("click", function () {
        applyAccount(state, provider.agent, account.name).then(reload).catch(function (e) {
          note(slot, String((e && e.message) || e));
        });
      });
    }
    wrap.appendChild(card);

    var del = el("button", "cma-trash");
    del.type = "button";
    del.title = "Sign out and delete " + account.name;
    del.setAttribute("aria-label", "Delete " + account.name);
    del.appendChild(icon("trash", 14));
    del.addEventListener("click", function () {
      slot.replaceChildren(confirmDelete(provider, account, function () {
        slot.replaceChildren(wrap);
      }));
    });
    wrap.appendChild(del);

    slot.appendChild(wrap);
    return slot;
  }

  /* Deleting signs the account out, removes its profile directory and drops any
   * routes pointing at it. None of that is undoable, so it gets a named
   * confirmation rather than a control that arms on a first click. */
  function confirmDelete(provider, account, cancel) {
    var box = el("div", "cma-confirm");
    box.appendChild(el("div", "cma-name", "Delete " + account.name + "?"));
    box.appendChild(el("div", "cma-sub",
      "Signs " + (account.email || account.name) +
      " out and drops every workspace routed to it."));

    var actions = el("div", "cma-actions");
    var no = el("button", "cma-act", "Cancel");
    no.type = "button";
    no.addEventListener("click", cancel);
    var yes = el("button", "cma-act cma-act-danger", "Delete");
    yes.type = "button";
    yes.addEventListener("click", function () {
      yes.disabled = true;
      yes.textContent = "Deleting…";
      acct("remove " + account.name + " " + provider.agent).then(reload).catch(function (e) {
        box.replaceChildren(el("div", "cma-sub", String((e && e.message) || e)));
      });
    });
    actions.appendChild(no);
    actions.appendChild(yes);
    box.appendChild(actions);
    return box;
  }

  function signInForm(agent, host, replaced) {
    var form = el("div", "cma-form");

    var name = document.createElement("input");
    name.className = "cma-input";
    name.placeholder = "name, for example work";
    name.spellcheck = false;

    var go = el("button", "cma-go", "Sign in");
    go.type = "button";

    var status = el("div", "cma-note", "Your browser opens for approval.");

    form.appendChild(name);
    form.appendChild(go);
    form.appendChild(status);
    setTimeout(function () { name.focus(); }, 0);

    var codeField = null;
    function fail(msg) { status.textContent = msg; go.disabled = false; }

    function poll(profile, tries) {
      acct("login-status " + profile + " " + agent)
        .then(function (out) {
          if (/^ok /.test(out)) {
            status.textContent = "Signed in as " + out.slice(3);
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
      var profile = name.value.trim();
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
    name.addEventListener("keydown", function (e) { if (e.key === "Enter") go.click(); });

    if (replaced && replaced.parentNode) replaced.parentNode.replaceChild(form, replaced);
    else host.appendChild(form);
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
    add.addEventListener("click", function () { signInForm(agent, host, add); });
    host.appendChild(add);

    host.appendChild(el("div", "cma-note", footText(state)));
  }

  function note(host, message) {
    var n = el("div", "cma-note", message);
    host.appendChild(n);
    setTimeout(function () { if (n.parentNode) n.parentNode.removeChild(n); }, 4000);
  }

  /* --------------------------------------------------------------- render -- */

  function render() {
    if (!open) return;
    var host = open.el;
    host.replaceChildren();
    if (open.view.level === "provider") providerView(open.state, host, open.view.agent);
    else rootView(open.state, host);
    place(host, open.anchor);
  }

  function errorPanel(anchor, e) {
    /* A dead button teaches nobody anything. Show what went wrong, in the same
     * panel the accounts would have appeared in. */
    var panel = el("div", "cma-panel");
    panel.appendChild(label("Accounts unavailable"));
    var n = el("div", "cma-note", String((e && e.message) || e));
    var code = el("code", "cma-code", CLI + " json");
    n.appendChild(code);
    panel.appendChild(n);
    seal(panel);
    mountFor(anchor).appendChild(panel);
    open = { el: panel, anchor: anchor, state: null, view: { level: "root" }, refresh: null };
    place(panel, anchor);
    listen();
    log("panel failed", e);
  }

  function listen() {
    setTimeout(function () {
      document.addEventListener("mousedown", onDocDown, true);
      document.addEventListener("keydown", onDocKey, true);
    }, 0);
  }

  function togglePanel(anchor, refresh) {
    if (open) {
      var same = open.anchor === anchor;
      closePanel();
      if (same) return;
    }
    anchor.setAttribute("aria-expanded", "true");
    var panel = el("div", "cma-panel");
    panel.setAttribute("role", "menu");
    seal(panel);
    loadState()
      .then(function (state) {
        mountFor(anchor).appendChild(panel);
        open = { el: panel, anchor: anchor, state: state, view: { level: "root" }, refresh: refresh };
        render();
        listen();
      })
      .catch(function (e) {
        anchor.setAttribute("aria-expanded", "false");
        errorPanel(anchor, e);
      });
  }

  function refreshTriggers() {
    var b = document.getElementById("cma-toolbar-btn");
    if (b) refreshToolbarLabel(b);
    refreshComposerChip();
    if (open && open.refresh) open.refresh();
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

    if (existing && existing.parentElement === host) {
      refreshToolbarLabel(existing);
      return;
    }
    if (existing && existing.parentNode) existing.parentNode.removeChild(existing);

    var btn = document.createElement("button");
    btn.id = "cma-toolbar-btn";
    btn.type = "button";
    btn.className = "cma-btn";
    btn.setAttribute("aria-label", "Agent account");
    btn.innerHTML = '<span class="cma-dot"></span><span class="cma-label">account</span>';
    seal(btn);
    btn.addEventListener("click", function (e) {
      e.preventDefault();
      togglePanel(btn, function () { refreshToolbarLabel(btn); });
    });

    if (before) host.insertBefore(btn, before);
    else host.appendChild(btn);
    refreshToolbarLabel(btn);
    log("toolbar button attached", before ? "next to Open in" : "floating (toolbar not found)");
  }

  function refreshToolbarLabel(btn) {
    loadState()
      .then(function (s) {
        var cur = primary(s);
        var lbl = btn.querySelector(".cma-label");
        var dot = btn.querySelector(".cma-dot");
        if (lbl) lbl.textContent = cur || (s.enabled ? "default" : "off");
        if (dot) dot.className = "cma-dot" + (s.enabled && cur ? "" : " cma-off");
        btn.title = cur ? "Agent account: " + cur : "No account chosen here";
      })
      .catch(function (e) {
        var lbl = btn.querySelector(".cma-label");
        if (lbl) lbl.textContent = "account?";
        btn.title = "conductor-acct did not answer: " + (e && e.message ? e.message : e);
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
    if (foot.querySelector("#cma-chip")) {
      refreshComposerChip();
      return;
    }

    var chip = document.createElement("button");
    chip.id = "cma-chip";
    chip.type = "button";
    chip.className = "cma-chip";
    chip.innerHTML = '<span class="cma-dot"></span><span class="cma-label">account</span>';
    seal(chip);
    chip.addEventListener("click", function (e) {
      e.preventDefault();
      togglePanel(chip, function () { refreshComposerChip(); });
    });
    foot.insertBefore(chip, foot.firstChild);
    refreshComposerChip();
    log("composer chip attached");
  }

  function refreshComposerChip() {
    var chip = document.getElementById("cma-chip");
    if (!chip) return;
    loadState()
      .then(function (s) {
        var lbl = chip.querySelector(".cma-label");
        var dot = chip.querySelector(".cma-dot");
        var name = primary(s) || "default account";
        if (lbl) lbl.textContent = name;
        if (dot) dot.className = "cma-dot" + (primary(s) ? "" : " cma-off");
        chip.title = "This workspace will run agents on: " + name;
      })
      .catch(function () {});
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
     * elements are already in place. */
    var pending = null;
    new MutationObserver(function () {
      if (pending) return;
      pending = setTimeout(function () {
        pending = null;
        tick();
      }, 250);
    }).observe(document.body, { childList: true, subtree: true });
    log("ready");
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    setTimeout(boot, 800);
  }
})();
