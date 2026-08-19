/* conductor-multi-account: the account panel injected into Conductor's
 * frontend by tools/patch-ui.py.
 *
 * GENERATED FILE. Do not edit. Source is src/panel/, styles are
 * src/panel/styles.scss, build with `pnpm build`.
 */
"use strict";
(() => {
  // src/panel/cli.ts
  var CLI = "$HOME/.conductor-accounts/bin/conductor-acct";
  function log(...parts) {
    if (window.__conductorMultiAccountDebug) {
      console.log("[multi-account]", ...parts);
    }
  }
  function sh(command) {
    const internals = window.__TAURI_INTERNALS__;
    if (!internals || !internals.invoke) {
      return Promise.reject(new Error("no Tauri bridge"));
    }
    return internals.invoke("execute_shell_command", { shell: "/bin/zsh", command, noRcs: true }).then((r) => {
      if (r && r.code !== 0) {
        throw new Error((r.stderr || "").trim() || "exit " + r.code);
      }
      return (r && r.stdout || "").trim();
    });
  }
  function acct(args) {
    return sh(CLI + " " + args);
  }
  function q(s) {
    return "'" + String(s).replace(/'/g, "'\\''") + "'";
  }
  function cliPath() {
    return CLI;
  }
  function message(e) {
    if (e instanceof Error) return e.message;
    return String(e);
  }

  // src/panel/state.ts
  var PLACES_TTL = 3e4;
  var placesCache = null;
  var placesAt = 0;
  function places() {
    if (placesCache && Date.now() - placesAt < PLACES_TTL) {
      return Promise.resolve(placesCache);
    }
    return Promise.all([
      acct("workspaces").catch(() => ""),
      acct("repos").catch(() => "")
    ]).then(([workspaces, repos]) => {
      const parse = (text, kind) => text.split("\n").map((l) => l.split("	")).filter((p) => p.length === 2 && p[0] && p[1]).map((p) => ({ kind, name: p[0], path: p[1] }));
      const list = parse(workspaces, "workspace").concat(parse(repos, "repository"));
      list.sort((a, b) => b.name.length - a.name.length);
      placesCache = list;
      placesAt = Date.now();
      return list;
    });
  }
  function chromeText() {
    const bits = [document.title || ""];
    const sel = "header,[class*=titlebar],[class*=toolbar],[data-tauri-drag-region]";
    const nodes = document.querySelectorAll(sel);
    for (let i = 0; i < nodes.length && i < 12; i++) {
      bits.push(nodes[i]?.textContent || "");
    }
    for (const id of ["cma-toolbar-btn", "cma-chip"]) {
      let node = document.getElementById(id);
      while (node && node !== document.body) {
        bits.push(node.textContent || "");
        if ((node.textContent || "").length > 400) break;
        node = node.parentElement;
      }
    }
    return bits.join(" \n ");
  }
  function currentTarget() {
    return places().then((list) => {
      const hay = chromeText();
      for (const place2 of list) {
        if (hay.indexOf(place2.name) >= 0) return place2;
      }
      return { kind: "none", name: "", path: "" };
    });
  }
  var STATE_TTL = 4e3;
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
    statePending = currentTarget().then(
      (target) => acct("json " + (target.path ? q(target.path) : "")).then((out) => {
        const st = JSON.parse(out);
        st.target = target;
        return st;
      })
    ).then(
      (st) => {
        stateCache = st;
        stateAt = Date.now();
        statePending = null;
        return st;
      },
      (e) => {
        statePending = null;
        throw e;
      }
    );
    return statePending;
  }
  function applyAccount(state, agent, profile) {
    const t = state.target;
    if (t.kind === "workspace") return acct(`use ${profile} ${agent} ${q(t.path)}`);
    if (t.kind === "repository") return acct(`bind ${profile} ${agent} ${q(t.path)}`);
    return Promise.reject(new Error("no workspace or repository in view"));
  }

  // src/panel/store.ts
  var panel = null;
  function setPanel(next) {
    panel = next;
  }
  var dialog = null;
  function openDialog() {
    return dialog;
  }
  function setDialog(next) {
    dialog = next;
  }
  function closeDialog() {
    if (dialog && dialog.parentNode) dialog.parentNode.removeChild(dialog);
    dialog = null;
  }
  var rerender = () => {
  };
  var refreshTriggers = () => {
  };
  function onRerender(fn) {
    rerender = fn;
  }
  function onRefreshTriggers(fn) {
    refreshTriggers = fn;
  }
  function redraw() {
    rerender();
  }
  function updateTriggers(state) {
    refreshTriggers(state);
  }
  function reload() {
    invalidate();
    return loadState(true).then((st) => {
      if (!panel) return;
      panel.state = st;
      rerender();
      refreshTriggers(st);
    });
  }

  // src/panel/attach.ts
  var SEALED = [
    "mousedown",
    "pointerdown",
    "mouseup",
    "pointerup",
    "click",
    "touchstart",
    "touchend"
  ];
  function seal(node) {
    for (const type of SEALED) {
      node.addEventListener(type, (e) => e.stopPropagation(), false);
    }
  }
  function mountFor(anchor) {
    let node = anchor;
    while (node && node !== document.body) {
      const role = node.getAttribute("role");
      if (node.tagName === "DIALOG" || role === "dialog" || role === "alertdialog" || node.getAttribute("aria-modal") === "true") {
        return node;
      }
      node = node.parentElement;
    }
    return document.body;
  }
  function place(node, anchor) {
    if (panel && panel.pos) {
      node.style.top = panel.pos.top + "px";
      node.style.left = panel.pos.left + "px";
      return;
    }
    const a = anchor.getBoundingClientRect();
    const h = node.offsetHeight;
    let wantTop = a.bottom + 6;
    if (wantTop + h > window.innerHeight - 12) {
      wantTop = Math.max(12, a.top - h - 6);
    }
    const wantLeft = Math.max(
      12,
      Math.min(a.left, window.innerWidth - node.offsetWidth - 12)
    );
    node.style.top = Math.round(wantTop) + "px";
    node.style.left = Math.round(wantLeft) + "px";
    const got = node.getBoundingClientRect();
    const dy = wantTop - got.top;
    const dx = wantLeft - got.left;
    const top = Math.round(wantTop + (Math.abs(dy) > 0.5 ? dy : 0));
    const left = Math.round(wantLeft + (Math.abs(dx) > 0.5 ? dx : 0));
    node.style.top = top + "px";
    node.style.left = left + "px";
    if (panel) panel.pos = { top, left };
  }

  // src/panel/dom.ts
  function el(tag, cls, text) {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }
  function label(text) {
    return el("div", "cma-head", text);
  }
  function cap(s) {
    const v = String(s || "");
    return v ? v.charAt(0).toUpperCase() + v.slice(1) : v;
  }
  var AGENT_LABEL = {
    claude: "Claude Code",
    codex: "Codex"
  };
  var AGENT_ICON = {
    claude: "claude",
    codex: "codex"
  };
  function primary(state) {
    const providers = state.providers || [];
    const claude = providers.filter((p) => p.agent === "claude")[0];
    if (claude && claude.current) return claude.current;
    const any = providers.filter((p) => p.current)[0];
    return any ? any.current : "";
  }
  function scopeText(state) {
    if (state.target.kind === "workspace") return "Workspace: " + state.target.name;
    if (state.target.kind === "repository") return "New workspaces in " + state.target.name;
    return "No workspace in view";
  }
  function footText(state) {
    if (state.target.kind === "workspace") {
      return "Applies to the next chat here. A chat already running keeps the account it started on.";
    }
    if (state.target.kind === "repository") {
      return "Applies to workspaces created from now on.";
    }
    return "Open a workspace to choose its account.";
  }
  function note(host, text) {
    const n = el("div", "cma-note", text);
    host.appendChild(n);
    setTimeout(() => {
      if (n.parentNode) n.parentNode.removeChild(n);
    }, 4e3);
  }

  // src/panel/icons.ts
  var SVG_NS = "http://www.w3.org/2000/svg";
  var CLAUDE_MARK = "m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z";
  var CODEX_MARK = "M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z";
  var PATHS = {
    chevron: ["M6 3.5 10.5 8 6 12.5"],
    back: ["M12.5 8H4", "M7.5 4.5 4 8l3.5 3.5"],
    /* Sign out, not a bin. The control signs the account out and leaves everything
     * else where it was, so a bin would promise deletion it does not do. Sign in is
     * the same arrow pointing the other way. */
    signout: ["M9.2 3.5H4.2v9h5", "M7.4 8h6.4", "M11.6 5.9 13.8 8l-2.2 2.1"],
    signin: ["M6.8 3.5h5v9h-5", "M8.6 8H2.2", "M4.4 5.9 2.2 8l2.2 2.1"],
    tick: ["M3.5 8.6 6.4 11.5 12.5 5"],
    plus: ["M8 3.5v9", "M3.5 8h9"],
    claude: [CLAUDE_MARK],
    codex: [CODEX_MARK]
  };
  var FILLED = /* @__PURE__ */ new Set(["claude", "codex"]);
  var GRID_24 = /* @__PURE__ */ new Set(["claude", "codex"]);
  function icon(name, size = 14) {
    const svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("viewBox", GRID_24.has(name) ? "0 0 24 24" : "0 0 16 16");
    svg.setAttribute("width", String(size));
    svg.setAttribute("height", String(size));
    if (FILLED.has(name)) {
      svg.setAttribute("fill", "currentColor");
    } else {
      svg.setAttribute("fill", "none");
      svg.setAttribute("stroke", "currentColor");
      svg.setAttribute("stroke-width", "1.4");
      svg.setAttribute("stroke-linecap", "round");
      svg.setAttribute("stroke-linejoin", "round");
    }
    svg.setAttribute("aria-hidden", "true");
    for (const d of PATHS[name] || []) {
      const p = document.createElementNS(SVG_NS, "path");
      p.setAttribute("d", d);
      svg.appendChild(p);
    }
    return svg;
  }

  // src/panel/views/root.ts
  function providerCard(provider) {
    const card = el("button", "cma-card");
    card.type = "button";
    card.appendChild(icon(AGENT_ICON[provider.agent] || "chevron", 13));
    const main = el("div", "cma-grow");
    main.appendChild(el("div", "cma-name", AGENT_LABEL[provider.agent] || provider.agent));
    const n = provider.accounts.length;
    main.appendChild(el("div", "cma-sub", n === 1 ? "1 Account" : n + " Accounts"));
    card.appendChild(main);
    const badge = provider.current ? provider.current.charAt(0).toUpperCase() + provider.current.slice(1) : provider.accounts.length ? "Not set" : "None";
    card.appendChild(el("span", "cma-badge", badge));
    card.appendChild(icon("chevron", 13));
    card.addEventListener("click", () => {
      if (!panel) return;
      panel.view = { level: "provider", agent: provider.agent };
      redraw();
    });
    return card;
  }
  function rootView(state, host) {
    host.appendChild(label(scopeText(state)));
    for (const p of state.providers) host.appendChild(providerCard(p));
    host.appendChild(el("div", "cma-sep"));
    const toggle = el("button", "cma-card");
    toggle.type = "button";
    const main = el("div", "cma-grow");
    main.appendChild(
      el("div", "cma-name", state.enabled ? "Turn routing off" : "Turn routing on")
    );
    main.appendChild(
      el(
        "div",
        "cma-sub",
        state.enabled ? "agents go back to one account" : "one account per workspace"
      )
    );
    toggle.appendChild(main);
    toggle.addEventListener("click", () => {
      acct(state.enabled ? "uninstall" : "install").then(() => reload()).catch((e) => note(host, message(e)));
    });
    host.appendChild(toggle);
    host.appendChild(el("div", "cma-note", footText(state)));
  }
  function loadingView(host) {
    host.appendChild(label("Loading accounts"));
    for (const agent of ["claude", "codex"]) {
      const card = el("div", "cma-card cma-ghost");
      card.appendChild(icon(AGENT_ICON[agent] || "chevron", 13));
      const main = el("div", "cma-grow");
      main.appendChild(el("div", "cma-name", AGENT_LABEL[agent] || agent));
      main.appendChild(el("div", "cma-sub", "reading accounts"));
      card.appendChild(main);
      host.appendChild(card);
    }
    host.appendChild(el("div", "cma-sep"));
    host.appendChild(
      el("div", "cma-note", "conductor-acct is answering. This is quick once warmed up.")
    );
  }
  function errorView(host, e) {
    host.appendChild(label("Accounts unavailable"));
    const n = el("div", "cma-note", message(e));
    n.appendChild(el("code", "cma-code", cliPath() + " json"));
    host.appendChild(n);
    log("panel failed", e);
  }

  // src/panel/dialog.ts
  function dialog2(opts) {
    closeDialog();
    const scrim = el("div", "cma-scrim");
    const box = el("div", "cma-dialog");
    box.setAttribute("role", "alertdialog");
    box.setAttribute("aria-modal", "true");
    seal(scrim);
    box.appendChild(el("div", "cma-name", opts.title));
    const body = el("div", "cma-sub", opts.body);
    box.appendChild(body);
    const actions = el("div", "cma-actions");
    const no = el("button", "cma-act", "Cancel");
    no.type = "button";
    const yes = el("button", "cma-act" + (opts.danger ? " cma-act-danger" : ""), opts.confirm);
    yes.type = "button";
    function shut() {
      document.removeEventListener("keydown", onKey, true);
      closeDialog();
    }
    function onKey(e) {
      if (e.key === "Escape") {
        e.stopPropagation();
        shut();
      }
    }
    no.addEventListener("click", shut);
    yes.addEventListener("click", () => {
      no.disabled = true;
      yes.disabled = true;
      yes.textContent = "Working…";
      opts.onConfirm(shut, (msg) => {
        yes.remove();
        no.disabled = false;
        no.textContent = "Close";
        body.textContent = msg;
      });
    });
    scrim.addEventListener("click", (e) => {
      if (e.target === scrim) shut();
    });
    actions.appendChild(no);
    actions.appendChild(yes);
    box.appendChild(actions);
    scrim.appendChild(box);
    (panel ? panel.el.parentNode : document.body)?.appendChild(scrim);
    setDialog(scrim);
    document.addEventListener("keydown", onKey, true);
    setTimeout(() => no.focus(), 0);
  }

  // src/panel/mask.ts
  function maskPart(s) {
    const n = s.length;
    if (n <= 2) return "**";
    if (n <= 5) return s.charAt(0) + "**";
    if (n <= 8) return s.slice(0, 2) + "**" + s.slice(-1);
    return s.slice(0, 3) + "**" + s.slice(-3);
  }
  function maskEmail(raw) {
    const s = String(raw || "");
    if (!s) return "";
    const at = s.lastIndexOf("@");
    if (at < 1) return maskPart(s);
    const local = s.slice(0, at);
    const domain = s.slice(at + 1);
    const dot = domain.indexOf(".");
    const host = dot > 0 ? domain.slice(0, dot) : domain;
    const suffix = dot > 0 ? domain.slice(dot) : "";
    return maskPart(local) + "@" + maskPart(host) + suffix;
  }

  // src/panel/views/sign-in.ts
  function duplicateOf(state, agent, profile, email) {
    if (!email || !state) return null;
    const mine = (state.providers || []).filter((p) => p.agent === agent)[0];
    if (!mine) return null;
    const clash = (mine.accounts || []).filter(
      (a) => a.name !== profile && a.email && a.email === email
    )[0];
    return clash ? clash.name : null;
  }
  function signInForm(agent, opts) {
    if (opts.host.querySelector(".cma-form")) return;
    const fixed = opts.profile || null;
    const form = el("div", "cma-form");
    let name = null;
    if (fixed) {
      form.appendChild(el("div", "cma-name", "Sign in to " + cap(fixed)));
    } else {
      name = document.createElement("input");
      name.className = "cma-input";
      name.placeholder = "name, for example work";
      name.spellcheck = false;
      form.appendChild(name);
    }
    const go = el("button", "cma-go", "Sign in");
    go.type = "button";
    const status = el("div", "cma-note", "Your browser opens for approval.");
    form.appendChild(go);
    form.appendChild(status);
    let codeField = null;
    function fail(msg) {
      status.textContent = msg;
      go.disabled = false;
    }
    function poll(profile, tries) {
      acct(`login-status ${profile} ${agent}`).then((out) => {
        if (/^ok\b/.test(out)) {
          const email = out.slice(2).trim();
          const clash = duplicateOf(opts.state, agent, profile, email);
          if (clash) {
            status.textContent = `${cap(clash)} is already signed in as ${maskEmail(email)}. One account cannot be two profiles, so they will sign each other out. Remove one with conductor-acct remove ${clash}.`;
            go.remove();
            if (codeField) codeField.remove();
            setTimeout(() => void reload(), 4e3);
            return;
          }
          status.textContent = email ? "Signed in as " + maskEmail(email) : "Signed in.";
          setTimeout(() => void reload(), 600);
          return;
        }
        if (/^error/.test(out)) {
          fail(out.replace(/^error\s*/, "") || "sign-in failed");
          return;
        }
        if (tries > 240) {
          fail("timed out waiting for the browser");
          return;
        }
        setTimeout(() => poll(profile, tries + 1), 1e3);
      }).catch((e) => fail(message(e)));
    }
    function askForCode(profile) {
      if (codeField) return;
      codeField = document.createElement("input");
      codeField.className = "cma-input";
      codeField.placeholder = "paste the code, then Enter";
      codeField.spellcheck = false;
      form.insertBefore(codeField, status);
      codeField.addEventListener("keydown", (e) => {
        if (e.key !== "Enter" || !codeField) return;
        const code = codeField.value.trim();
        if (!code) return;
        status.textContent = "Checking…";
        acct(`login-code ${profile} ${q(code)}`).then(() => poll(profile, 0)).catch((err) => fail(message(err)));
      });
      setTimeout(() => codeField?.focus(), 0);
    }
    go.addEventListener("click", () => {
      const profile = fixed || (name ? name.value.trim() : "");
      if (!/^[A-Za-z0-9_-]+$/.test(profile)) {
        fail("Letters, digits, - and _ only.");
        return;
      }
      go.disabled = true;
      status.textContent = "Starting sign-in…";
      acct(`login-start ${profile} ${agent}`).then((url) => {
        status.textContent = "Approve in your browser, then paste the code.";
        sh("open " + q(url)).catch(() => {
        });
        askForCode(profile);
        poll(profile, 0);
      }).catch((e) => fail(message(e)));
    });
    if (name) {
      name.addEventListener("keydown", (e) => {
        if (e.key === "Enter") go.click();
      });
      setTimeout(() => name?.focus(), 0);
    } else {
      setTimeout(() => go.focus(), 0);
    }
    if (opts.replace && opts.replace.parentNode) {
      opts.replace.parentNode.replaceChild(form, opts.replace);
    } else {
      opts.host.appendChild(form);
    }
  }

  // src/panel/views/account-row.ts
  function accountSlot(state, provider, account) {
    const slot = el("div", "cma-slot");
    const row = el("div", "cma-row2");
    const card = el("button", "cma-card");
    card.type = "button";
    card.setAttribute("role", "menuitemradio");
    card.setAttribute("aria-checked", account.active ? "true" : "false");
    const main = el("div", "cma-grow");
    const shown = account.email ? maskEmail(account.email) : cap(account.name);
    const line = el("div", "cma-name" + (account.email ? " cma-mask" : ""), shown);
    if (account.email) line.setAttribute("aria-label", "email hidden");
    main.appendChild(line);
    main.appendChild(
      el(
        "div",
        "cma-sub",
        account.email ? cap(account.name) : account.signedIn ? "Signed in" : "Not signed in"
      )
    );
    card.appendChild(main);
    const tickslot = el("div", "cma-tickslot");
    if (account.active) tickslot.appendChild(icon("tick", 13));
    card.appendChild(tickslot);
    if (state.target.kind === "none") {
      row.setAttribute("aria-disabled", "true");
      card.setAttribute("aria-disabled", "true");
      card.title = "Open a workspace, or the New Workspace dialog, to pick an account";
    } else {
      card.addEventListener("click", () => {
        applyAccount(state, provider.agent, account.name).then(() => reload()).catch((e) => note(panel ? panel.el : row, message(e)));
      });
    }
    row.appendChild(card);
    if (account.signedIn) {
      const out = el("button", "cma-signout");
      out.type = "button";
      out.title = "Sign out of " + cap(account.name);
      out.setAttribute("aria-label", "Sign out of " + cap(account.name));
      out.appendChild(icon("signout", 14));
      out.addEventListener("click", () => confirmSignOut(provider, account));
      row.appendChild(out);
    } else {
      const back = el("button", "cma-signout cma-signin");
      back.type = "button";
      back.title = "Sign in to " + cap(account.name);
      back.setAttribute("aria-label", "Sign in to " + cap(account.name));
      back.appendChild(icon("signin", 14));
      back.addEventListener("click", () => {
        signInForm(provider.agent, { host: slot, profile: account.name, state });
      });
      row.appendChild(back);
    }
    slot.appendChild(row);
    return slot;
  }
  function confirmSignOut(provider, account) {
    dialog2({
      title: "Sign out of " + cap(account.name) + "?",
      body: "Signs " + (account.email ? maskEmail(account.email) : cap(account.name)) + " out of " + (AGENT_LABEL[provider.agent] || provider.agent) + ". Nothing else changes: the account stays in this list, and its routes, sessions and transcripts are untouched. Sign back in from here whenever you like.",
      confirm: "Sign out",
      danger: true,
      onConfirm: (done, fail) => {
        acct(`logout ${account.name} ${provider.agent}`).then(() => {
          done();
          void reload();
        }).catch((e) => fail(message(e)));
      }
    });
  }

  // src/panel/views/provider.ts
  function providerView(state, host, agent) {
    const provider = state.providers.filter((p) => p.agent === agent)[0];
    if (!provider) {
      if (panel) panel.view = { level: "root" };
      rootView(state, host);
      return;
    }
    const back = el("button", "cma-back");
    back.type = "button";
    back.appendChild(icon("back", 13));
    back.appendChild(el("span", null, "Back"));
    back.addEventListener("click", () => {
      if (!panel) return;
      panel.view = { level: "root" };
      redraw();
    });
    host.appendChild(back);
    const title = el("div", "cma-title");
    title.appendChild(icon(AGENT_ICON[agent] || "chevron", 13));
    title.appendChild(el("span", null, AGENT_LABEL[agent] || agent));
    host.appendChild(title);
    if (!provider.accounts.length) {
      host.appendChild(
        el("div", "cma-note", "No accounts yet. Add one and it becomes selectable here.")
      );
    }
    for (const account of provider.accounts) {
      host.appendChild(accountSlot(state, provider, account));
    }
    const add = el("button", "cma-add");
    add.type = "button";
    add.appendChild(icon("plus", 12));
    add.appendChild(el("span", null, "Add new account"));
    add.addEventListener("click", () => {
      signInForm(agent, { host, replace: add, profile: null, state });
    });
    host.appendChild(add);
    host.appendChild(el("div", "cma-note", footText(state)));
  }

  // src/panel/controller.ts
  function closePanel() {
    closeDialog();
    if (panel?.el.parentNode) panel.el.parentNode.removeChild(panel.el);
    panel?.anchor.setAttribute("aria-expanded", "false");
    setPanel(null);
    document.removeEventListener("mousedown", onDocDown, true);
    document.removeEventListener("keydown", onDocKey, true);
  }
  function onDocDown(e) {
    if (!panel || openDialog()) return;
    const target = e.target;
    if (panel.el.contains(target)) return;
    if (panel.anchor.contains(target)) return;
    closePanel();
  }
  function onDocKey(e) {
    if (e.key !== "Escape" || !panel || openDialog()) return;
    if (panel.view.level === "provider") {
      panel.view = { level: "root" };
      render();
      return;
    }
    closePanel();
  }
  function render() {
    if (!panel) return;
    const host = panel.el;
    host.replaceChildren();
    if (panel.error) errorView(host, panel.error);
    else if (!panel.state) loadingView(host);
    else if (panel.view.level === "provider" && panel.view.agent) {
      providerView(panel.state, host, panel.view.agent);
    } else rootView(panel.state, host);
    place(host, panel.anchor);
  }
  onRerender(render);
  function listen() {
    setTimeout(() => {
      document.addEventListener("mousedown", onDocDown, true);
      document.addEventListener("keydown", onDocKey, true);
    }, 0);
  }
  function togglePanel(anchor) {
    if (panel) {
      const same = panel.anchor === anchor;
      closePanel();
      if (same) return;
    }
    anchor.setAttribute("aria-expanded", "true");
    const node = el("div", "cma-panel");
    node.setAttribute("role", "menu");
    seal(node);
    mountFor(anchor).appendChild(node);
    setPanel({ el: node, anchor, state: null, error: null, view: { level: "root" } });
    render();
    listen();
    loadState().then((state) => {
      if (!panel || panel.el !== node) return;
      panel.state = state;
      render();
      updateTriggers(state);
    }).catch((e) => {
      if (!panel || panel.el !== node) return;
      panel.error = e;
      render();
    });
  }
  function openOnPress(trigger) {
    let pressedAt = 0;
    trigger.addEventListener("pointerdown", (e) => {
      if (e.button !== void 0 && e.button !== 0) return;
      e.preventDefault();
      pressedAt = Date.now();
      togglePanel(trigger);
    });
    trigger.addEventListener("click", (e) => {
      e.preventDefault();
      if (Date.now() - pressedAt < 700) return;
      togglePanel(trigger);
    });
  }

  // src/panel/triggers.ts
  function findOpenIn() {
    const nodes = document.querySelectorAll(
      "button,[role=button],a,[data-slot=button]"
    );
    for (const n of nodes) {
      const lbl = (n.getAttribute("aria-label") || n.getAttribute("title") || n.getAttribute("data-tooltip") || "").trim();
      if (/open (in|remote)/i.test(lbl)) return n;
    }
    for (const n of nodes) {
      const t = (n.textContent || "").trim();
      if (t.length < 24 && /open in/i.test(t)) return n;
    }
    let icon2 = document.querySelector(
      'img[src*="app-icons"],img[src*="finder.png"]'
    );
    while (icon2 && icon2 !== document.body) {
      if (icon2.tagName === "BUTTON" || icon2.getAttribute("role") === "button") return icon2;
      icon2 = icon2.parentElement;
    }
    return null;
  }
  function floatingHost() {
    const existing = document.getElementById("cma-float");
    if (existing) return existing;
    const host = el("div");
    host.id = "cma-float";
    host.style.cssText = "position:fixed;top:9px;right:14px;z-index:99998";
    document.body.appendChild(host);
    return host;
  }
  var missedToolbar = 0;
  function toolbarButton() {
    const existing = document.getElementById("cma-toolbar-btn");
    const anchor = findOpenIn();
    let host;
    let before = null;
    if (anchor && anchor.parentElement) {
      host = anchor.parentElement;
      before = anchor;
      missedToolbar = 0;
    } else {
      if (++missedToolbar < 8) return;
      host = floatingHost();
    }
    if (existing && existing.isConnected && existing.parentElement === host) return;
    if (existing && existing.parentNode) existing.parentNode.removeChild(existing);
    const btn = el("button", "cma-btn");
    btn.id = "cma-toolbar-btn";
    btn.type = "button";
    btn.setAttribute("aria-label", "Agent account");
    btn.hidden = true;
    btn.appendChild(el("span", "cma-label"));
    openOnPress(btn);
    if (before) host.insertBefore(btn, before);
    else host.appendChild(btn);
    refreshToolbarLabel(btn);
  }
  function refreshToolbarLabel(btn, state) {
    const apply = (s) => {
      const cur = primary(s);
      const lbl = btn.querySelector(".cma-label");
      if (lbl) lbl.textContent = cap(cur) || (s.enabled ? "Default" : "Off");
      btn.title = cur ? "Agent account: " + cap(cur) : "No account chosen here";
      btn.hidden = false;
    };
    if (state) {
      apply(state);
      return;
    }
    loadState().then(apply).catch((e) => {
      const lbl = btn.querySelector(".cma-label");
      if (lbl) lbl.textContent = "Account?";
      btn.title = "conductor-acct did not answer: " + message(e);
      btn.hidden = false;
    });
  }
  function findComposer() {
    const els = document.querySelectorAll("[placeholder],[data-placeholder]");
    for (const node of els) {
      const p = node.getAttribute("placeholder") || node.getAttribute("data-placeholder") || "";
      if (/what do you want to work on/i.test(p)) return node;
    }
    return null;
  }
  function composerFooter(node) {
    let e = node;
    for (let i = 0; i < 8 && e; i++, e = e.parentElement) {
      const rows = e.querySelectorAll(":scope > div");
      if (rows.length >= 2) {
        const last = rows[rows.length - 1];
        if (last && last.querySelector("button") && last.textContent.length < 400) {
          return last;
        }
      }
    }
    return null;
  }
  function composerChip() {
    const composer = findComposer();
    if (!composer) return;
    const foot = composerFooter(composer);
    if (!foot) return;
    if (foot.querySelector("#cma-chip")) return;
    const chip = el("button", "cma-chip");
    chip.id = "cma-chip";
    chip.type = "button";
    chip.hidden = true;
    chip.appendChild(el("span", "cma-label"));
    openOnPress(chip);
    foot.insertBefore(chip, foot.firstChild);
    refreshComposerChip();
  }
  function refreshComposerChip(state) {
    const chip = document.getElementById("cma-chip");
    if (!chip) return;
    const apply = (s) => {
      const lbl = chip.querySelector(".cma-label");
      const name = cap(primary(s)) || "Default account";
      if (lbl) lbl.textContent = name;
      chip.title = "This workspace will run agents on: " + name;
      chip.hidden = false;
    };
    if (state) {
      apply(state);
      return;
    }
    loadState().then(apply).catch(() => {
    });
  }
  function refreshTriggers2(state) {
    const btn = document.getElementById("cma-toolbar-btn");
    if (btn) refreshToolbarLabel(btn, state);
    refreshComposerChip(state);
  }

  // scss-text:/Users/utsav/conductor/workspaces/conductor-playground/antananarivo/conductor-multi-account/src/panel/styles.scss
  var styles_default = ".cma-btn,.cma-chip{display:inline-flex;align-items:center;gap:6px;height:28px;padding:0 9px;border:0;border-radius:calc(var(--radius) - 2px);background:rgba(0,0,0,0);color:var(--muted-foreground);font-size:12px;font-weight:500;line-height:1;white-space:nowrap;cursor:pointer;transition:background .12s,color .12s}.cma-btn:hover,.cma-btn[aria-expanded=true],.cma-chip:hover,.cma-chip[aria-expanded=true]{background:var(--accent);color:var(--foreground)}.cma-btn[hidden],.cma-chip[hidden]{display:none}.cma-panel{position:fixed;z-index:99999;box-sizing:border-box;width:300px;max-height:min(70vh,560px);padding:6px;overflow-y:auto;overscroll-behavior:contain;border:1px solid var(--border);border-radius:var(--radius);background:var(--popover);color:var(--popover-foreground);box-shadow:0 10px 38px rgba(0,0,0,.28),0 2px 8px rgba(0,0,0,.16);font-size:13px;transform-origin:top left;animation:cma-in .11s ease-out}.cma-panel svg{flex:none;display:block}@keyframes cma-in{from{opacity:0;transform:scale(0.97) translateY(-2px)}to{opacity:1;transform:none}}.cma-head{padding:6px 6px 8px;color:var(--muted-foreground);font-size:11px;font-weight:500}.cma-note{padding:8px 6px 4px;color:var(--muted-foreground);font-size:11px;line-height:1.5}.cma-code{display:block;margin-top:5px;padding:5px 7px;border-radius:calc(var(--radius) - 3px);background:var(--muted);color:var(--foreground);font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px;user-select:all}.cma-sep{height:1px;margin:6px 2px;background:var(--border)}.cma-card{display:flex;align-items:center;gap:9px;box-sizing:border-box;width:100%;margin:0 0 5px;padding:8px 10px;border:1px solid var(--border);border-radius:calc(var(--radius) - 2px);background:rgba(0,0,0,0);color:inherit;font:inherit;text-align:left;outline:0;cursor:pointer;transition:background .12s,border-color .12s}.cma-card:last-child{margin-bottom:0}.cma-card:hover,.cma-card:focus-visible{background:var(--accent);border-color:var(--ring, var(--border))}.cma-card[aria-disabled=true]{opacity:.45;cursor:not-allowed}.cma-card[aria-disabled=true]:hover{background:rgba(0,0,0,0);border-color:var(--border)}.cma-card[aria-checked=true] .cma-name{font-weight:600}.cma-ghost{opacity:.4;cursor:default;animation:cma-pulse 1.1s ease-in-out infinite}@keyframes cma-pulse{0%,100%{opacity:.4}50%{opacity:.62}}.cma-grow{flex:1;min-width:0}.cma-name{overflow:hidden;font-size:13px;line-height:1.35;text-overflow:ellipsis;white-space:nowrap}.cma-sub{margin-top:1px;overflow:hidden;color:var(--muted-foreground);font-size:11px;line-height:1.35;text-overflow:ellipsis;white-space:nowrap}.cma-badge{max-width:96px;overflow:hidden;color:var(--muted-foreground);font-size:11px;text-overflow:ellipsis;white-space:nowrap}.cma-tickslot{flex:none;display:flex;justify-content:center;width:15px;color:var(--foreground);opacity:.9}.cma-row2{display:flex;align-items:stretch;box-sizing:border-box;width:100%;margin:0 0 5px;overflow:hidden;border:1px solid var(--border);border-radius:calc(var(--radius) - 2px);transition:border-color .12s}.cma-row2:last-child{margin-bottom:0}.cma-row2:hover{border-color:var(--ring, var(--border))}.cma-row2 .cma-card{flex:1;width:auto;min-width:0;margin:0;border:0;border-radius:0}.cma-row2[aria-disabled=true]{opacity:.45}.cma-row2:hover .cma-signout{opacity:.85}.cma-signout{flex:none;display:inline-flex;align-items:center;justify-content:center;align-self:stretch;width:36px;border:0;border-left:1px solid var(--border);border-radius:0;background:rgba(0,0,0,0);color:var(--muted-foreground);opacity:.6;cursor:pointer;transition:opacity .12s,background .12s,color .12s}.cma-signout:hover,.cma-signout:focus-visible{opacity:1;background:var(--destructive, #ff5a5a);color:#fff}.cma-signin:hover,.cma-signin:focus-visible{background:var(--accent);color:var(--foreground)}.cma-slot{margin-bottom:5px}.cma-slot:last-child{margin-bottom:0}.cma-slot .cma-row2{margin-bottom:0}.cma-slot .cma-form{margin-top:5px}.cma-mask{font-variant-numeric:tabular-nums;letter-spacing:.01em}.cma-back{display:inline-flex;align-items:center;gap:6px;margin:0 0 6px;padding:4px 6px;border:0;border-radius:6px;background:rgba(0,0,0,0);color:var(--muted-foreground);font:inherit;font-size:12px;cursor:pointer;transition:background .12s,color .12s}.cma-back:hover{background:var(--accent);color:var(--foreground)}.cma-title{display:flex;align-items:center;gap:8px;padding:0 6px 9px;font-size:12px;font-weight:600}.cma-add{display:flex;align-items:center;justify-content:center;gap:7px;box-sizing:border-box;width:100%;margin-top:6px;padding:8px 10px;border:1px dashed var(--border);border-radius:calc(var(--radius) - 2px);background:rgba(0,0,0,0);color:var(--muted-foreground);font:inherit;font-size:12px;cursor:pointer;transition:background .12s,color .12s,border-color .12s}.cma-add:hover{background:var(--accent);color:var(--foreground);border-color:var(--ring, var(--border))}.cma-form{display:flex;flex-direction:column;gap:6px;margin-top:6px;padding:8px;border:1px solid var(--border);border-radius:calc(var(--radius) - 2px)}.cma-form .cma-note{padding:0}.cma-input{box-sizing:border-box;width:100%;height:30px;padding:0 9px;border:1px solid var(--input-border, var(--border));border-radius:calc(var(--radius) - 3px);background:var(--input, transparent);color:var(--foreground);font-size:12px;outline:0}.cma-input:focus{border-color:var(--popover-ring, var(--ring))}.cma-go{height:30px;border:0;border-radius:calc(var(--radius) - 3px);background:var(--foreground);color:var(--background);font-size:12px;font-weight:500;cursor:pointer}.cma-go:disabled{opacity:.5;cursor:default}.cma-scrim{position:fixed;inset:0;z-index:100000;display:flex;align-items:center;justify-content:center;padding:24px;background:rgba(0,0,0,.45);animation:cma-fade .12s ease-out}@keyframes cma-fade{from{opacity:0}to{opacity:1}}.cma-dialog{box-sizing:border-box;width:300px;max-width:100%;padding:14px;border:1px solid var(--border);border-radius:var(--radius);background:var(--popover);color:var(--popover-foreground);box-shadow:0 18px 48px rgba(0,0,0,.4);font-size:13px;animation:cma-pop .12s ease-out}.cma-dialog .cma-name{font-size:13px;font-weight:600;white-space:normal}.cma-dialog .cma-sub{margin-top:6px;line-height:1.5;white-space:normal}@keyframes cma-pop{from{opacity:0;transform:scale(0.96)}to{opacity:1;transform:none}}.cma-actions{display:flex;gap:7px;margin-top:13px}.cma-act{flex:1;height:30px;border:1px solid var(--border);border-radius:calc(var(--radius) - 3px);background:rgba(0,0,0,0);color:inherit;font:inherit;font-size:12px;font-weight:500;cursor:pointer}.cma-act:hover{background:var(--accent)}.cma-act:disabled{opacity:.6;cursor:default}.cma-act-danger{border-color:rgba(0,0,0,0);background:var(--destructive, #ff5a5a);color:#fff}.cma-act-danger:hover{background:var(--destructive, #ff5a5a);filter:brightness(1.08)}";

  // src/panel/index.ts
  var VERSION = "0.2.0";
  function injectStyles() {
    if (document.getElementById("cma-style")) return;
    const style = document.createElement("style");
    style.id = "cma-style";
    style.textContent = styles_default;
    document.head.appendChild(style);
  }
  function tick() {
    try {
      injectStyles();
      toolbarButton();
      composerChip();
    } catch (e) {
      log("tick failed", e);
    }
  }
  function boot() {
    onRefreshTriggers(refreshTriggers2);
    tick();
    let pending = null;
    new MutationObserver(() => {
      if (pending) return;
      pending = setTimeout(() => {
        pending = null;
        tick();
      }, 250);
    }).observe(document.body, { childList: true, subtree: true });
    setInterval(() => {
      if (!document.getElementById("cma-toolbar-btn") && !document.getElementById("cma-chip")) {
        return;
      }
      loadState().then(refreshTriggers2).catch(() => {
      });
    }, 8e3);
    log("ready");
  }
  if (!window.__conductorMultiAccount) {
    window.__conductorMultiAccount = { version: VERSION };
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", boot);
    } else {
      setTimeout(boot, 800);
    }
  }
})();
