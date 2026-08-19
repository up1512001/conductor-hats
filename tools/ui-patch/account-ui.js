/* conductor-multi-account: account UI injected into Conductor's frontend.
 *
 * Appended to Conductor's main bundle by tools/patch-ui.py. It adds:
 *
 *   - a button in the workspace toolbar, next to "Open in", opening a panel to
 *     switch, add, remove and enable/disable accounts
 *   - an account row in the New Workspace composer, so a workspace starts on
 *     the account you meant
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
  window.__conductorMultiAccount = { version: "0.1.0" };

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

  /* Conductor routes as /repository/<id>/workspace/<id>, and the router keys on
   * the directory, so the id has to be turned back into a path. */
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
   * app's palette, radii and light/dark handling instead of approximating them. */
  var CSS = [
    ".cma-btn{display:inline-flex;align-items:center;gap:6px;height:28px;padding:0 9px;",
    "border-radius:calc(var(--radius) - 2px);background:transparent;border:0;",
    "color:var(--muted-foreground);font-size:12px;font-weight:500;line-height:1;",
    "white-space:nowrap;cursor:default;transition:background .12s,color .12s}",
    ".cma-btn:hover{background:var(--accent);color:var(--foreground)}",
    ".cma-btn[aria-expanded=true]{background:var(--accent);color:var(--foreground)}",
    ".cma-dot{width:6px;height:6px;border-radius:50%;background:currentColor;flex:none;opacity:.75}",
    ".cma-dot.cma-off{opacity:.35}",

    ".cma-panel{position:fixed;z-index:99999;min-width:264px;max-width:328px;padding:4px;",
    "border-radius:var(--radius);border:1px solid var(--border);",
    "background:var(--popover);color:var(--popover-foreground);",
    "box-shadow:0 10px 38px rgb(0 0 0/.28),0 2px 8px rgb(0 0 0/.16);",
    "font-size:13px;transform-origin:top left;animation:cma-in .11s ease-out}",
    "@keyframes cma-in{from{opacity:0;transform:scale(.97) translateY(-2px)}",
    "to{opacity:1;transform:none}}",

    ".cma-head{padding:8px 8px 5px;font-size:11px;font-weight:500;",
    "color:var(--muted-foreground)}",
    ".cma-row{display:flex;align-items:center;gap:8px;padding:6px 8px;",
    "border-radius:calc(var(--radius) - 3px);cursor:default;outline:0}",
    ".cma-row:hover,.cma-row:focus-visible{background:var(--popover-accent,var(--accent))}",
    ".cma-row[aria-disabled=true]{opacity:.4}",
    ".cma-row[aria-disabled=true]:hover{background:transparent}",
    ".cma-name{font-size:13px;line-height:1.3}",
    ".cma-mail{font-size:11px;line-height:1.3;margin-top:1px;color:var(--muted-foreground)}",
    ".cma-tick{margin-left:auto;font-size:11px;opacity:.85}",
    ".cma-sep{height:1px;margin:4px 6px;background:var(--border)}",
    ".cma-note{padding:6px 8px 7px;font-size:11px;line-height:1.5;",
    "color:var(--muted-foreground)}",
    ".cma-code{display:block;margin-top:5px;padding:5px 7px;",
    "border-radius:calc(var(--radius) - 3px);background:var(--muted);",
    "color:var(--foreground);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;",
    "font-size:11px;user-select:all}",

    ".cma-chip{display:inline-flex;align-items:center;gap:6px;height:28px;padding:0 9px;",
    "border-radius:calc(var(--radius) - 2px);background:transparent;border:0;",
    "color:var(--muted-foreground);font-size:12px;font-weight:500;line-height:1;",
    "cursor:default;transition:background .12s,color .12s}",
    ".cma-chip:hover,.cma-chip[aria-expanded=true]{background:var(--accent);",
    "color:var(--foreground)}",

    ".cma-add{padding:6px 8px 4px;display:flex;flex-direction:column;gap:6px}",
    ".cma-add .cma-note{padding:0}",
    ".cma-input{width:100%;height:30px;padding:0 9px;box-sizing:border-box;",
    "border-radius:calc(var(--radius) - 3px);border:1px solid var(--input-border,var(--border));",
    "background:var(--input,transparent);color:var(--foreground);font-size:12px;outline:0}",
    ".cma-input:focus{border-color:var(--popover-ring,var(--ring))}",
    ".cma-go{height:30px;border:0;border-radius:calc(var(--radius) - 3px);",
    "background:var(--foreground);color:var(--background);font-size:12px;font-weight:500;",
    "cursor:default}",
    ".cma-go:disabled{opacity:.5}",

    ".cma-grow{flex:1;min-width:0}",
    ".cma-provider{gap:6px;font-weight:500}",
    ".cma-caret{display:inline-block;width:10px;text-align:center;opacity:.55;",
    "transition:transform .13s}",
    ".cma-provider[aria-expanded=true] .cma-caret{transform:rotate(90deg)}",
    ".cma-mark{width:14px;text-align:center;opacity:.8}",
    ".cma-badge{font-size:11px;color:var(--muted-foreground);white-space:nowrap;",
    "max-width:110px;overflow:hidden;text-overflow:ellipsis}",
    ".cma-body{padding-left:10px}",
    ".cma-acct{padding-left:14px}",
    ".cma-acct[aria-checked=true] .cma-name{font-weight:600}",
    ".cma-x{margin-left:6px;flex:none;border:0;background:transparent;padding:2px 5px;",
    "border-radius:5px;color:var(--muted-foreground);font-size:11px;line-height:1;",
    "opacity:0;cursor:default;transition:opacity .12s}",
    ".cma-row:hover .cma-x{opacity:.7}",
    ".cma-x:hover{opacity:1;background:var(--accent);color:var(--foreground)}",
    ".cma-x-armed{opacity:1;color:#ff6b6b}",
    ".cma-add-row{padding-left:14px;color:var(--muted-foreground);font-size:12px}",
    ".cma-add-row:hover{color:var(--foreground)}"
  ].join("");

  function injectCss() {
    if (document.getElementById("cma-style")) return;
    var el = document.createElement("style");
    el.id = "cma-style";
    el.textContent = CSS;
    document.head.appendChild(el);
  }

  /* ---------------------------------------------------------------- panel -- */

  var openPanel = null;
  var lastTrigger = null;

  function closePanel() {
    if (openPanel && openPanel.parentNode) openPanel.parentNode.removeChild(openPanel);
    openPanel = null;
    if (lastTrigger) lastTrigger.setAttribute("aria-expanded", "false");
    lastTrigger = null;
    document.removeEventListener("mousedown", onDocDown, true);
    document.removeEventListener("keydown", onDocKey, true);
  }

  function onDocDown(e) {
    if (!openPanel) return;
    if (openPanel.contains(e.target)) return;
    if (lastTrigger && lastTrigger.contains(e.target)) return;
    closePanel();
  }

  function onDocKey(e) {
    if (e.key === "Escape") closePanel();
  }

  function row(opts) {
    var el = document.createElement("div");
    el.className = "cma-row";
    el.setAttribute("role", "menuitem");
    if (opts.disabled) el.setAttribute("aria-disabled", "true");

    var main = document.createElement("div");
    var name = document.createElement("div");
    name.className = "cma-name";
    name.textContent = opts.title;
    main.appendChild(name);
    if (opts.subtitle) {
      var sub = document.createElement("div");
      sub.className = "cma-mail";
      sub.textContent = opts.subtitle;
      main.appendChild(sub);
    }
    el.appendChild(main);

    if (opts.tick) {
      var t = document.createElement("div");
      t.className = "cma-tick";
      t.textContent = "✓";
      el.appendChild(t);
    }
    if (opts.onClick && !opts.disabled) el.addEventListener("click", opts.onClick);
    return el;
  }

  function separator() {
    var s = document.createElement("div");
    s.className = "cma-sep";
    return s;
  }

  var AGENT_LABEL = { claude: "Claude Code", codex: "Codex" };
  var AGENT_MARK = { claude: "✳", codex: "◆" };

  /* The label shows Claude's account, falling back to whichever provider has
   * one, because that is the one people mean when they glance at it. */
  function primary(state) {
    var claude = (state.providers || []).filter(function (p) { return p.agent === "claude"; })[0];
    if (claude && claude.current) return claude.current;
    var any = (state.providers || []).filter(function (p) { return p.current; })[0];
    return any ? any.current : "";
  }

  function sectionLabel(text) {
    var el = document.createElement("div");
    el.className = "cma-head";
    el.textContent = text;
    return el;
  }

  function addAccountFlow(host, agent, onDone) {
    if (host.querySelector(".cma-add")) return;
    var wrap = document.createElement("div");
    wrap.className = "cma-add";

    var name = document.createElement("input");
    name.className = "cma-input";
    name.placeholder = "name, for example work";
    name.spellcheck = false;

    var status = document.createElement("div");
    status.className = "cma-note";
    status.textContent = "Sign-in opens your browser.";

    var go = document.createElement("button");
    go.className = "cma-go";
    go.type = "button";
    go.textContent = "Sign in";

    wrap.appendChild(name);
    wrap.appendChild(go);
    wrap.appendChild(status);
    host.appendChild(wrap);
    setTimeout(function () { name.focus(); }, 0);

    var codeField = null;
    function fail(msg) { status.textContent = msg; go.disabled = false; }

    function poll(profile, tries) {
      acct("login-status " + profile + " " + agent)
        .then(function (out) {
          if (/^ok /.test(out)) {
            status.textContent = "Signed in as " + out.slice(3);
            setTimeout(function () { closePanel(); if (onDone) onDone(); }, 700);
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
            wrap.insertBefore(codeField, status);
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
  }

  function accountRow(state, provider, account, onChanged) {
    var el = document.createElement("div");
    el.className = "cma-row cma-acct";
    el.setAttribute("role", "menuitemradio");
    el.setAttribute("aria-checked", account.active ? "true" : "false");

    var main = document.createElement("div");
    main.className = "cma-grow";
    var n = document.createElement("div");
    n.className = "cma-name";
    n.textContent = account.name;
    var m = document.createElement("div");
    m.className = "cma-mail";
    m.textContent = account.email || "not signed in";
    main.appendChild(n);
    main.appendChild(m);
    el.appendChild(main);

    if (account.active) {
      var tick = document.createElement("span");
      tick.className = "cma-tick";
      tick.textContent = "✓";
      el.appendChild(tick);
    }

    var del = document.createElement("button");
    del.type = "button";
    del.className = "cma-x";
    del.textContent = "✕";
    del.title = "Sign out and delete " + account.name;
    del.addEventListener("click", function (e) {
      e.stopPropagation();
      if (del.dataset.armed !== "1") {
        del.dataset.armed = "1";
        del.textContent = "remove?";
        del.classList.add("cma-x-armed");
        setTimeout(function () {
          del.dataset.armed = "";
          del.textContent = "✕";
          del.classList.remove("cma-x-armed");
        }, 3000);
        return;
      }
      acct("remove " + account.name + " " + provider.agent).then(function () {
        closePanel();
        if (onChanged) onChanged();
      });
    });
    el.appendChild(del);

    if (state.target.kind === "none") {
      el.setAttribute("aria-disabled", "true");
    } else {
      el.addEventListener("click", function () {
        applyAccount(state, provider.agent, account.name)
          .then(function () { closePanel(); if (onChanged) onChanged(); })
          .catch(function (e) { log("switch failed", e); });
      });
    }
    return el;
  }

  var expanded = { claude: true, codex: false };

  function providerGroup(state, provider, onChanged) {
    var wrap = document.createElement("div");
    wrap.className = "cma-group";

    var header = document.createElement("div");
    header.className = "cma-row cma-provider";
    header.setAttribute("role", "button");
    header.setAttribute("aria-expanded", expanded[provider.agent] ? "true" : "false");

    var caret = document.createElement("span");
    caret.className = "cma-caret";
    caret.textContent = "›";
    var mark = document.createElement("span");
    mark.className = "cma-mark";
    mark.textContent = AGENT_MARK[provider.agent] || "•";
    var title = document.createElement("span");
    title.className = "cma-grow cma-name";
    title.textContent = AGENT_LABEL[provider.agent] || provider.agent;
    var badge = document.createElement("span");
    badge.className = "cma-badge";
    badge.textContent = provider.current || (provider.accounts.length ? "not set" : "none");

    header.appendChild(caret);
    header.appendChild(mark);
    header.appendChild(title);
    header.appendChild(badge);
    wrap.appendChild(header);

    var body = document.createElement("div");
    body.className = "cma-body";
    wrap.appendChild(body);

    provider.accounts.forEach(function (a) {
      body.appendChild(accountRow(state, provider, a, onChanged));
    });

    var add = document.createElement("div");
    add.className = "cma-row cma-add-row";
    add.setAttribute("role", "menuitem");
    add.textContent = "+  Add account";
    add.addEventListener("click", function () { addAccountFlow(body, provider.agent, onChanged); });
    body.appendChild(add);

    function sync() {
      var open = expanded[provider.agent];
      header.setAttribute("aria-expanded", open ? "true" : "false");
      body.style.display = open ? "" : "none";
    }
    header.addEventListener("click", function () {
      expanded[provider.agent] = !expanded[provider.agent];
      sync();
    });
    sync();
    return wrap;
  }

  /* Conductor's New Workspace modal closes on any pointer event outside itself,
   * and a panel parked on document.body counts as outside: choosing an account
   * dismissed the modal and lost the typed prompt. Mounting inside the dialog
   * puts it within that containment check, and swallowing pointer events at the
   * panel edge covers listeners that test coordinates instead. */
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

  function sealPointerEvents(el) {
    ["mousedown", "pointerdown", "mouseup", "pointerup", "click", "touchstart"]
      .forEach(function (type) {
        el.addEventListener(type, function (e) { e.stopPropagation(); }, true);
      });
  }

  function buildPanel(state, anchor, onChanged) {
    var panel = document.createElement("div");
    panel.className = "cma-panel";
    panel.setAttribute("role", "menu");
    sealPointerEvents(panel);

    var scope = state.target.kind === "workspace"
      ? "Workspace: " + state.target.name
      : state.target.kind === "repository"
        ? "New workspaces in " + state.target.name
        : "No workspace in view";
    panel.appendChild(sectionLabel(scope));

    state.providers.forEach(function (p) {
      panel.appendChild(providerGroup(state, p, onChanged));
    });

    panel.appendChild(separator());
    panel.appendChild(
      row({
        title: state.enabled ? "Turn routing off" : "Turn routing on",
        subtitle: state.enabled ? "agents go back to one account" : "one account per workspace",
        onClick: function () {
          acct(state.enabled ? "uninstall" : "install").then(function () {
            closePanel();
            if (onChanged) onChanged();
          });
        }
      })
    );

    var foot = document.createElement("div");
    foot.className = "cma-note";
    foot.textContent = state.target.kind === "workspace"
      ? "Applies to the next chat here. A chat already running keeps the account it started on."
      : state.target.kind === "repository"
        ? "Applies to workspaces created from now on."
        : "Open a workspace to choose its account.";
    panel.appendChild(foot);

    mountFor(anchor).appendChild(panel);
    var r = anchor.getBoundingClientRect();
    var top = r.bottom + 6;
    if (top + panel.offsetHeight > window.innerHeight - 12)
      top = Math.max(12, r.top - panel.offsetHeight - 6);
    panel.style.top = Math.round(top) + "px";
    panel.style.left =
      Math.round(Math.max(12, Math.min(r.left, window.innerWidth - panel.offsetWidth - 12))) + "px";

    openPanel = panel;
    setTimeout(function () {
      document.addEventListener("mousedown", onDocDown, true);
      document.addEventListener("keydown", onDocKey, true);
    }, 0);
    return panel;
  }

  function togglePanel(anchor, refresh) {
    if (openPanel) {
      closePanel();
      return;
    }
    anchor.setAttribute("aria-expanded", "true");
    lastTrigger = anchor;
    loadState()
      .then(function (state) {
        buildPanel(state, anchor, refresh);
      })
      .catch(function (e) {
        /* A dead button teaches nobody anything. Show what went wrong, in the
         * same panel the accounts would have appeared in. */
        var panel = document.createElement("div");
        panel.className = "cma-panel";
        var head = document.createElement("div");
        head.className = "cma-head";
        head.textContent = "Accounts unavailable";
        panel.appendChild(head);
        var note = document.createElement("div");
        note.className = "cma-note";
        note.textContent = String((e && e.message) || e);
        var code = document.createElement("code");
        code.className = "cma-code";
        code.textContent = CLI + " json";
        note.appendChild(code);
        panel.appendChild(note);
        sealPointerEvents(panel);
        mountFor(anchor).appendChild(panel);
        var r = anchor.getBoundingClientRect();
        panel.style.top = Math.round(r.bottom + 6) + "px";
        panel.style.left =
          Math.round(Math.max(12, Math.min(r.left, window.innerWidth - panel.offsetWidth - 12))) + "px";
        openPanel = panel;
        setTimeout(function () {
          document.addEventListener("mousedown", onDocDown, true);
          document.addEventListener("keydown", onDocKey, true);
        }, 0);
        log("panel failed", e);
      });
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
      var label = (n.getAttribute("aria-label") || n.getAttribute("title") ||
                   n.getAttribute("data-tooltip") || "").trim();
      if (/open (in|remote)/i.test(label)) return n;
    }
    for (var j = 0; j < nodes.length; j++) {
      var t = (nodes[j].textContent || "").trim();
      if (t.length < 24 && /open in/i.test(t)) return nodes[j];
    }
    /* Last resort, and in practice the reliable one: the control renders the
     * chosen app's icon from /app-icons/, so find that image and climb to the
     * button wrapping it. */
    var icon = document.querySelector('img[src*="app-icons"],img[src*="finder.png"]');
    while (icon && icon !== document.body) {
      if (icon.tagName === "BUTTON" || icon.getAttribute("role") === "button") return icon;
      icon = icon.parentElement;
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
    sealPointerEvents(btn);
    btn.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
      togglePanel(btn, function () {
        refreshToolbarLabel(btn);
        refreshComposerChip();
      });
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
        var label = btn.querySelector(".cma-label");
        var dot = btn.querySelector(".cma-dot");
        if (label) label.textContent = cur || (s.enabled ? "default" : "off");
        if (dot) dot.className = "cma-dot" + (s.enabled && cur ? "" : " cma-off");
        btn.title = cur ? "Agent account: " + cur : "No account chosen here";
      })
      .catch(function (e) {
        var label = btn.querySelector(".cma-label");
        if (label) label.textContent = "account?";
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
    var el = node;
    for (var i = 0; i < 8 && el; i++, el = el.parentElement) {
      var rows = el.querySelectorAll(":scope > div");
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
    sealPointerEvents(chip);
    chip.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
      togglePanel(chip, function () {
        refreshComposerChip();
        var b = document.getElementById("cma-toolbar-btn");
        if (b) refreshToolbarLabel(b);
      });
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
        var label = chip.querySelector(".cma-label");
        var dot = chip.querySelector(".cma-dot");
        var name = primary(s) || "default account";
        if (label) label.textContent = name;
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
