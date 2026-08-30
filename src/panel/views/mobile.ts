/** Conductor-native setup, pairing QR, copying and revocation for mobile access. */

import { acct, message, q } from "../cli.js";
import { el, note } from "../dom.js";
import { dialog } from "../dialog.js";
import { icon } from "../icons.js";
import { publishModelCatalog } from "../model_catalog.js";
import { qrCode } from "../qr.js";
import { fromToolbar } from "../route.js";
import { panel } from "../store.js";
import { refreshMobileTrigger } from "../triggers.js";

interface Pairing {
  origin: string;
  path: string;
  url: string;
  expires_at: number;
}

interface MobileStatus {
  origin: string;
  pairing: Pairing | null;
  service: ServiceStatus;
}

interface ServiceStatus {
  running: boolean;
  address: string;
  connections: number;
  source: string;
}

let workspaceScope = "";

function mobileCommand(name: string): Promise<string> {
  return acct("remote " + name + " " + q(workspaceScope));
}

interface PairingResult {
  pairing: Pairing;
  service: ServiceStatus;
}

function active(host: HTMLElement): boolean {
  return !!panel && panel.view.level === "mobile" && host.isConnected;
}

function frame(host: HTMLElement): void {
  host.replaceChildren();
  const title = el("div", "cma-title");
  title.append(icon("phone", 14), el("span", null, "Mobile access"));
  host.append(title);
}

function copy(value: string, button: HTMLButtonElement): void {
  const fallback = (): void => {
    const field = document.createElement("textarea");
    field.value = value;
    field.style.position = "fixed";
    field.style.opacity = "0";
    document.body.appendChild(field);
    field.select();
    document.execCommand("copy");
    field.remove();
  };
  const task = navigator.clipboard?.writeText(value);
  (task || Promise.resolve().then(fallback))
    .then(() => {
      button.textContent = "Copied";
      setTimeout(() => {
        if (button.isConnected) button.textContent = "Copy link";
      }, 1600);
    })
    .catch(() => {
      fallback();
      button.textContent = "Copied";
    });
}

function addressForm(host: HTMLElement, existing = ""): void {
  frame(host);
  host.appendChild(
    el(
      "div",
      "cma-note cma-mobile-intro",
      "Enter the stable HTTPS address mapped to hats on this Mac. The listener stays on loopback."
    )
  );
  const form = el("div", "cma-form");
  const input = document.createElement("input");
  input.className = "cma-input";
  input.placeholder = "https://conductor.example.com";
  input.value = existing;
  input.spellcheck = false;
  input.setAttribute("aria-label", "Public HTTPS address");
  const save = el("button", "cma-go", "Save address");
  save.type = "button";
  const status = el("div", "cma-note", "A named tunnel supplies this address; hats does not open a router port.");
  const submit = (): void => {
    if (!input.value.trim() || save.disabled) return;
    save.disabled = true;
    status.textContent = "Checking address…";
    acct("remote mobile-origin " + q(input.value.trim()) + " " + q(workspaceScope))
      .then((raw) => {
        if (!active(host)) return;
        const result = JSON.parse(raw) as { origin: string; service: ServiceStatus };
        ready(host, result.origin, result.service);
      })
      .catch((error) => {
        status.textContent = message(error);
        save.disabled = false;
      });
  };
  save.addEventListener("click", submit);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") submit();
  });
  form.append(input, save, status);
  host.appendChild(form);
  setTimeout(() => input.focus(), 0);
}

function addressRow(origin: string): HTMLElement {
  const row = el("div", "cma-mobile-address");
  const copy = el("div", "cma-grow");
  copy.append(el("div", "cma-name", "Public address"), el("div", "cma-sub", origin));
  row.append(icon("link", 13), copy);
  return row;
}

function serviceRow(service: ServiceStatus): HTMLElement {
  const row = el("div", "cma-mobile-service");
  const dot = el("span", service.running ? "cma-service-dot is-running" : "cma-service-dot");
  const copy = el("div", "cma-grow");
  copy.append(
    el(
      "div",
      "cma-name",
      service.connections > 0
        ? `${service.connections} phone${service.connections === 1 ? "" : "s"} connected`
        : service.running
          ? "Ready for a phone"
          : "Mobile access stopped"
    ),
    el("div", "cma-sub", (service.source || "Conductor") + " · " + (service.running ? `Protected at ${service.address}` : "Not connected"))
  );
  row.append(dot, copy);
  return row;
}

function stopButton(host: HTMLElement, origin: string): HTMLButtonElement {
  const stop = el("button", "cma-mobile-danger", "Stop mobile access");
  stop.type = "button";
  stop.addEventListener("click", () => {
    dialog({
      title: "Stop mobile access?",
      body: "The local service will stop and every paired phone will be disconnected and signed out.",
      confirm: "Stop access",
      danger: true,
      onConfirm: (done, fail) => {
        mobileCommand("mobile-stop")
          .then((raw) => {
            const service = JSON.parse(raw) as ServiceStatus;
            done();
            refreshMobileTrigger(true);
            if (active(host)) ready(host, origin, service);
          })
          .catch((error) => fail(message(error)));
      },
    });
  });
  return stop;
}

function ready(host: HTMLElement, origin: string, service: ServiceStatus): void {
  frame(host);
  host.append(addressRow(origin));
  host.append(serviceRow(service));
  const create = el("button", "cma-go cma-mobile-primary", "Create pairing code");
  create.type = "button";
  create.addEventListener("click", () => {
    create.disabled = true;
    create.textContent = "Starting secure service…";
    mobileCommand("mobile-pair")
      .then((raw) => {
        const result = JSON.parse(raw) as PairingResult;
        publishModelCatalog(true);
        refreshMobileTrigger(true);
        if (active(host)) pairingView(host, result.pairing, result.service);
      })
      .catch((error) => {
        create.disabled = false;
        create.textContent = "Create pairing code";
        note(host, message(error));
      });
  });
  const change = el("button", "cma-mobile-secondary", "Change address");
  change.type = "button";
  change.addEventListener("click", () => addressForm(host, origin));
  host.append(create, change);
  if (service.running) host.appendChild(stopButton(host, origin));
  host.appendChild(
    el("div", "cma-note", "Creating a code starts the protected loopback service automatically. Each code gets a fresh 64-character path; its secret stays after # and never reaches proxy or server logs.")
  );
}

function expiry(host: HTMLElement, pairing: Pairing): HTMLElement {
  const label = el("span", "cma-pair-expiry");
  const tick = (): void => {
    if (!active(host) || !label.isConnected) return;
    const seconds = Math.max(0, pairing.expires_at - Math.floor(Date.now() / 1000));
    label.textContent = seconds ? `Expires in ${Math.ceil(seconds / 60)} min` : "Expired";
    if (seconds) setTimeout(tick, 1000);
  };
  setTimeout(tick, 0);
  return label;
}

function pairingView(host: HTMLElement, pairing: Pairing, service: ServiceStatus): void {
  frame(host);
  host.append(addressRow(pairing.origin));
  host.append(serviceRow(service));
  const qr = el("div", "cma-qr-wrap");
  qr.appendChild(qrCode(pairing.url));
  host.appendChild(qr);
  const state = el("div", "cma-pair-state");
  state.append(el("span", "cma-pair-dot"), el("span", null, "One-use link"), expiry(host, pairing));
  host.appendChild(state);
  const link = el("div", "cma-pair-link");
  const shortPath = pairing.path.slice(0, 9) + "…" + pairing.path.slice(-8);
  link.appendChild(el("code", null, pairing.origin + shortPath + "#token=••••••••"));
  const copyButton = el("button", null, "Copy link");
  copyButton.type = "button";
  copyButton.addEventListener("click", () => copy(pairing.url, copyButton));
  link.appendChild(copyButton);
  host.appendChild(link);
  const revoke = el("button", "cma-mobile-danger", "Revoke paired phones");
  revoke.type = "button";
  revoke.addEventListener("click", () => {
    dialog({
      title: "Revoke mobile access?",
      body: "Every paired browser will be signed out. A replacement one-use code will be created.",
      confirm: "Revoke phones",
      danger: true,
      onConfirm: (done, fail) => {
        mobileCommand("mobile-revoke")
          .then((raw) => {
            const result = JSON.parse(raw) as PairingResult;
            done();
            publishModelCatalog(true);
            refreshMobileTrigger(true);
            if (active(host)) pairingView(host, result.pairing, result.service);
          })
          .catch((error) => fail(message(error)));
      },
    });
  });
  const change = el("button", "cma-mobile-secondary", "Change public address");
  change.type = "button";
  change.addEventListener("click", () => addressForm(host, pairing.origin));
  host.append(revoke, change, stopButton(host, pairing.origin));
  host.appendChild(el("div", "cma-note", "Scan with the phone camera, or copy the link to the phone. Keep this code private."));
}

export function mobileView(host: HTMLElement): void {
  frame(host);
  workspaceScope = fromToolbar().workspace || "";
  if (!workspaceScope) {
    host.appendChild(el("div", "cma-note", "Open a workspace to connect this Conductor app to your phone."));
    return;
  }
  host.appendChild(el("div", "cma-note", "Reading mobile access…"));
  mobileCommand("mobile-status")
    .then((raw) => {
      if (!active(host)) return;
      const status = JSON.parse(raw) as MobileStatus;
      if (!status.origin) addressForm(host);
      else if (status.pairing && status.service.running) {
        pairingView(host, status.pairing, status.service);
      } else ready(host, status.origin, status.service);
    })
    .catch((error) => {
      if (!active(host)) return;
      frame(host);
      host.appendChild(el("div", "cma-note", message(error)));
    });
}
