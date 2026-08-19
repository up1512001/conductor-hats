/**
 * A confirmation dialog, mounted in the same host as the panel and sealing its own
 * edge, because Conductor dismisses on pointer events it reads as outside.
 */

import { el } from "./dom.js";
import { seal } from "./attach.js";
import { closeDialog, panel, setDialog } from "./store.js";

export interface DialogOptions {
  title: string;
  body: string;
  confirm: string;
  danger?: boolean;
  onConfirm: (done: () => void, fail: (message: string) => void) => void;
}

export function dialog(opts: DialogOptions): void {
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

  function shut(): void {
    document.removeEventListener("keydown", onKey, true);
    closeDialog();
  }

  function onKey(e: KeyboardEvent): void {
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
