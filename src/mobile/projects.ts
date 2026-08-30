/** Groups flat Conductor chat rows into the repository hierarchy the phone uses. */

import type { Chat, Project } from "./types.js";

export function groupProjects(chats: Chat[]): Project[] {
  const grouped = new Map<string, Project>();
  for (const chat of chats) {
    const key = chat.repository_id || chat.project_path || chat.project || "projects";
    if (!grouped.has(key)) grouped.set(key, { key, name: chat.project || "Projects", chats: [] });
    grouped.get(key)?.chats.push(chat);
  }
  return Array.from(grouped.values());
}
