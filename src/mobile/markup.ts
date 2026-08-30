/** Escaping, small-subset Markdown and shared glyphs for the phone client. */

const entities: Record<string, string> = {
  "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
};

export const esc = (value: unknown): string => String(value ?? "").replace(
  /[&<>"']/g,
  (char) => entities[char] || char
);

export const chevron =
  '<svg class="chev" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5l7 7-7 7"/></svg>';

export function when(raw: string): string {
  const date = new Date(raw);
  return Number.isNaN(date.getTime())
    ? ""
    : date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function inline(text: string): string {
  return esc(text)
    .replace(/`([^`\n]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*\n]+)\*/g, "<em>$1</em>");
}

export function markdown(source: unknown): string {
  const code: string[] = [];
  const held = String(source ?? "").replace(/```([^\n]*)\n?([\s\S]*?)```/g, (_, lang: string, body: string) => {
    const index = code.push(`<pre><code data-language="${esc(lang.trim())}">${esc(body.replace(/\n$/, ""))}</code></pre>`) - 1;
    return `\n\n\u0001${index}\u0002\n\n`;
  });
  return held.split(/\n{2,}/).filter(Boolean).map((block) => {
    const token = block.trim().match(/^\u0001(\d+)\u0002$/);
    if (token) return code[Number(token[1])] || "";
    const lines = block.split("\n");
    const first = lines[0] || "";
    if (/^#{1,3} /.test(first)) {
      const level = Math.min(3, first.match(/^#+/)?.[0].length || 1);
      return `<h${level}>${inline(first.replace(/^#{1,3} /, ""))}</h${level}>`;
    }
    if (lines.every((line) => /^\s*[-*] /.test(line))) {
      return lines.map((line) => `<div class="md-li">• ${inline(line.replace(/^\s*[-*] /, ""))}</div>`).join("");
    }
    if (lines.every((line) => /^> ?/.test(line))) {
      return `<blockquote>${lines.map((line) => inline(line.replace(/^> ?/, ""))).join("<br>")}</blockquote>`;
    }
    return `<p>${lines.map(inline).join("<br>")}</p>`;
  }).join("");
}
