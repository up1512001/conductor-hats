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
  function currentWorkspacePath() {
    var m = location.hash.match(/workspace\/([0-9a-f-]{36})/i) ||
            location.pathname.match(/workspace\/([0-9a-f-]{36})/i);
    if (!m) return Promise.resolve("");
    return acct("resolve " + m[1]).catch(function () { return ""; });
  }

  function loadState() {
    return currentWorkspacePath().then(function (path) {
      return acct("json " + (path ? "'" + path.replace(/'/g, "'\\''") + "'" : "")).then(function (out) {
        var s = JSON.parse(out);
        s.path = path;
        return s;
      });
    });
  }

  /* ------------------------------------------------------------------ css -- */

  var CSS = [
    ".cma-btn{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 8px;",
    "border-radius:6px;border:1px solid rgba(255,255,255,.12);background:rgba(255,255,255,.04);",
    "color:inherit;font-size:12px;line-height:1;cursor:default;white-space:nowrap}",
    ".cma-btn:hover{background:rgba(255,255,255,.09)}",
    ".cma-dot{width:7px;height:7px;border-radius:50%;background:#5b9dff;flex:none}",
    ".cma-dot.cma-off{background:#8b8b8b}",
    ".cma-panel{position:fixed;z-index:99999;min-width:270px;max-width:340px;padding:6px;",
    "border-radius:10px;border:1px solid rgba(255,255,255,.14);background:#1c1c1e;",
    "box-shadow:0 12px 40px rgba(0,0,0,.5);font-size:12px;color:#e8e8ea}",
    ".cma-head{padding:7px 9px 5px;font-size:11px;text-transform:uppercase;letter-spacing:.05em;opacity:.5}",
    ".cma-row{display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:7px;cursor:default}",
    ".cma-row:hover{background:rgba(255,255,255,.08)}",
    ".cma-row[aria-disabled=true]{opacity:.45}",
    ".cma-name{font-weight:600}",
    ".cma-mail{opacity:.55;font-size:11px;margin-top:1px}",
    ".cma-tick{margin-left:auto;opacity:.9}",
    ".cma-sep{height:1px;margin:5px 4px;background:rgba(255,255,255,.1)}",
    ".cma-note{padding:6px 9px 8px;opacity:.55;font-size:11px;line-height:1.45}",
    ".cma-code{display:block;margin-top:5px;padding:5px 7px;border-radius:5px;",
    "background:rgba(255,255,255,.07);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;",
    "font-size:11px;user-select:all}",
    ".cma-chip{display:inline-flex;align-items:center;gap:5px;height:24px;padding:0 8px;",
    "border-radius:6px;font-size:12px;color:inherit;opacity:.85;cursor:default}",
    ".cma-chip:hover{background:rgba(255,255,255,.08);opacity:1}"
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

  function closePanel() {
    if (openPanel && openPanel.parentNode) openPanel.parentNode.removeChild(openPanel);
    openPanel = null;
    document.removeEventListener("mousedown", onDocDown, true);
    document.removeEventListener("keydown", onDocKey, true);
  }

  function onDocDown(e) {
    if (openPanel && !openPanel.contains(e.target)) closePanel();
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

  function buildPanel(state, anchor, onChanged) {
    var panel = document.createElement("div");
    panel.className = "cma-panel";
    panel.setAttribute("role", "menu");

    var head = document.createElement("div");
    head.className = "cma-head";
    head.textContent = state.path ? state.path.split("/").pop() : "Accounts";
    panel.appendChild(head);

    if (!state.accounts.length) {
      panel.appendChild(row({ title: "No accounts yet", disabled: true }));
    }

    state.accounts.forEach(function (a) {
      panel.appendChild(
        row({
          title: a.name,
          subtitle: a.email || "not signed in",
          tick: a.active,
          disabled: !state.path,
          onClick: function () {
            acct("use " + a.name + " claude '" + state.path.replace(/'/g, "'\\''") + "'")
              .then(function () {
                closePanel();
                if (onChanged) onChanged();
              })
              .catch(function (e) {
                log("switch failed", e);
              });
          }
        })
      );
    });

    panel.appendChild(separator());

    /* Signing in needs a browser round trip and a TTY, so the panel shows the
     * command rather than pretending it can do it. */
    var addRow = row({ title: "Add an account", subtitle: "needs a terminal" });
    addRow.addEventListener("click", function () {
      if (addRow.nextSibling && addRow.nextSibling.className === "cma-note") return;
      var note = document.createElement("div");
      note.className = "cma-note";
      note.textContent = "Signing in opens a browser, so run this in a terminal:";
      var code = document.createElement("code");
      code.className = "cma-code";
      code.textContent = "conductor-acct add <name>";
      note.appendChild(code);
      addRow.parentNode.insertBefore(note, addRow.nextSibling);
    });
    panel.appendChild(addRow);

    if (state.accounts.length) {
      panel.appendChild(
        row({
          title: "Remove an account",
          subtitle: "signs out and deletes the profile",
          onClick: function () {
            var names = state.accounts.map(function (a) { return a.name; });
            var pick = window.prompt("Remove which account?\n\n" + names.join("\n"), names[0]);
            if (!pick || names.indexOf(pick) < 0) return;
            acct("remove " + pick).then(function () {
              closePanel();
              if (onChanged) onChanged();
            });
          }
        })
      );
    }

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
    foot.textContent = "Applies to the next chat in this workspace. A chat already " +
      "running keeps the account it started on.";
    panel.appendChild(foot);

    document.body.appendChild(panel);
    var r = anchor.getBoundingClientRect();
    panel.style.top = Math.round(r.bottom + 6) + "px";
    var left = Math.min(r.left, window.innerWidth - panel.offsetWidth - 12);
    panel.style.left = Math.round(Math.max(12, left)) + "px";

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
    loadState()
      .then(function (state) {
        buildPanel(state, anchor, refresh);
      })
      .catch(function (e) {
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
        var label = btn.querySelector(".cma-label");
        var dot = btn.querySelector(".cma-dot");
        if (label) label.textContent = s.current || (s.enabled ? "default" : "off");
        if (dot) dot.className = "cma-dot" + (s.enabled && s.current ? "" : " cma-off");
        btn.title = s.current
          ? "Agent account: " + s.current
          : "No account chosen for this workspace";
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
        var name = s.current || "default account";
        if (label) label.textContent = name;
        if (dot) dot.className = "cma-dot" + (s.current ? "" : " cma-off");
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
