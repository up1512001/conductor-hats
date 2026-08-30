/** Wire state shared by the typed mobile transport and renderers. */

export interface Chat {
  project: string;
  project_path: string;
  repository_id: string;
  workspace: string;
  workspace_id: string;
  path: string;
  session: string;
  agent: string;
  status: string;
  unread: number;
  title: string;
  context: number;
  context_tokens: number;
  model: string;
  permission: string;
  effort: string;
  personality: string;
  fast: boolean;
  updated_at: string;
  pending: number;
  on: string;
  next: string;
}

export interface TranscriptLine {
  kind: string;
  role: string;
  at: string;
  name: string;
  text: string;
  detail: string;
  failed: boolean;
}

export interface OutboxItem {
  message: string;
  state: string;
}

export interface Account {
  name: string;
  label: string;
  signed_in: boolean;
}

export interface ActiveChat {
  session: string;
  transcript: TranscriptLine[];
  outbox: OutboxItem[];
  controls: Array<{
    id: string;
    setting: string;
    value: string;
    state: string;
    result: string;
    error: string;
  }>;
  creates: Array<{
    id: string;
    state: string;
    result: string;
    error: string;
  }>;
}

export interface MobileSnapshot {
  type?: string;
  source: string;
  chats: Chat[];
  active: ActiveChat | null;
  accounts: Record<string, Account[]>;
  models: Record<string, string[]>;
}

export interface Project {
  key: string;
  name: string;
  chats: Chat[];
}

export type MobileCommand = Record<string, string> & { type: string };
