/** A high-contrast SVG QR code, following T3 Code's share-panel treatment. */

import qrFactory from "qrcode-generator";

const NS = "http://www.w3.org/2000/svg";

export function qrCode(value: string): SVGSVGElement {
  const code = qrFactory(0, "M");
  code.addData(value);
  code.make();
  const margin = 2;
  const count = code.getModuleCount();
  const size = count + margin * 2;
  const commands: string[] = [];

  for (let row = 0; row < count; row += 1) {
    let start = -1;
    for (let column = 0; column <= count; column += 1) {
      const dark = column < count && code.isDark(row, column);
      if (dark && start < 0) start = column;
      if (dark || start < 0) continue;
      commands.push(`M${start + margin} ${row + margin}h${column - start}v1H${start + margin}z`);
      start = -1;
    }
  }

  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${size} ${size}`);
  svg.setAttribute("width", "168");
  svg.setAttribute("height", "168");
  svg.setAttribute("shape-rendering", "crispEdges");
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", "Pairing link, scan to connect this phone");
  const background = document.createElementNS(NS, "rect");
  background.setAttribute("width", String(size));
  background.setAttribute("height", String(size));
  background.setAttribute("fill", "#fff");
  const modules = document.createElementNS(NS, "path");
  modules.setAttribute("d", commands.join(""));
  modules.setAttribute("fill", "#000");
  svg.append(background, modules);
  return svg;
}
