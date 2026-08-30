//! Every chat Conductor has open, and the account each one is on.
//!
//! Two accounts per chat, and the difference matters. `on` is what the process
//! already running took when it spawned and cannot be changed. `next` is what
//! the process after it will take. They differ exactly when a chat has been
//! pointed somewhere new and not yet restarted.

use crate::{places, remote, routes, session, settings, store};

#[derive(serde::Serialize)]
struct Wire<'a> {
    project: &'a str,
    project_path: &'a str,
    repository_id: &'a str,
    workspace: &'a str,
    workspace_id: &'a str,
    path: &'a str,
    session: &'a str,
    agent: &'a str,
    status: &'a str,
    unread: i64,
    title: &'a str,
    context: f64,
    context_tokens: i64,
    model: &'a str,
    permission: &'a str,
    effort: &'a str,
    personality: &'a str,
    fast: bool,
    updated_at: &'a str,
    pending: usize,
    on: &'a str,
    next: &'a str,
}

/// Non-archived workspaces, visible chats, in the order Conductor lists them.
///
/// Conductor's sidebar puts the newest workspace at the top and numbers the
/// first nine for its shortcuts, and inside one it keeps chats in the order they
/// were started. Ordering by last activity instead reshuffled the list under the
/// reader every time an agent wrote a line, so the same chat was never twice in
/// the same place.
///
/// `is_hidden` covers the chats Conductor keeps but does not show, which would
/// otherwise pad the list with rows nobody recognises.
const CHATS: &str = "select w.directory_name, w.workspace_path, s.id, \
     coalesce(nullif(s.claude_session_id,''), s.id), coalesce(s.agent_type,'claude'), \
     coalesce(s.status,''), coalesce(s.unread_count,0), \
     replace(coalesce(nullif(s.title,''),'Untitled'), '|', ' '), \
     coalesce(s.context_used_percent,0) \
   from sessions s join workspaces w on w.id = s.workspace_id \
   where w.state != 'archived' and coalesce(s.is_hidden,0) = 0 \
     and w.workspace_path is not null \
   order by s.updated_at desc";

/// Newer presentation fields are optional because hats also reads older and
/// copied Conductor databases. A failed detailed query falls back to `CHATS`,
/// which is why `created_at` is only ordered on here: a database old enough to
/// lack it still lists its chats, just in last-activity order.
const CHATS_DETAIL: &str = "select w.directory_name, w.workspace_path, s.id, \
     coalesce(nullif(s.claude_session_id,''), s.id), coalesce(s.agent_type,'claude'), \
     coalesce(s.status,''), coalesce(s.unread_count,0), \
     replace(coalesce(nullif(s.title,''),'Untitled'), '|', ' '), \
     coalesce(s.context_used_percent,0), coalesce(s.context_token_count,0), \
     coalesce(s.model,''), coalesce(s.permission_mode,''), \
     coalesce(nullif(s.claude_effort_level,''),s.codex_thinking_level,''), \
     coalesce(s.agent_personality,''), coalesce(s.fast_mode,0), coalesce(s.updated_at,''), \
     w.id, coalesce(w.repository_id,''), coalesce(r.name,''), coalesce(r.root_path,''), \
     replace(coalesce(nullif(w.workspace_name,''),nullif(w.branch,''),w.directory_name),'|',' ') \
   from sessions s join workspaces w on w.id = s.workspace_id \
   left join repos r on r.id = w.repository_id \
   where w.state != 'archived' and coalesce(s.is_hidden,0) = 0 \
     and w.workspace_path is not null \
   order by w.created_at desc, s.created_at asc";

pub struct Chat {
    pub project: String,
    pub project_path: String,
    pub repository_id: String,
    pub workspace: String,
    pub workspace_id: String,
    pub path: String,
    pub session: String,
    pub agent: String,
    pub status: String,
    pub unread: i64,
    pub title: String,
    pub context: f64,
    pub context_tokens: i64,
    pub model: String,
    pub permission: String,
    pub effort: String,
    pub personality: String,
    pub fast: bool,
    pub updated_at: String,
    /// The account the running process took. Empty when it has not started, or
    /// when it started before hats began recording this.
    pub on: String,
    /// The account the next process will take.
    pub next: String,
}

/// Splits on the separator `sqlite3` writes between columns. Titles are the only
/// free text here and the query has already had the separator removed from them.
fn parse(line: &str) -> Option<Chat> {
    let f: Vec<&str> = line.split('|').collect();
    if f.len() < 9 {
        return None;
    }
    let agent = f[4].to_string();
    let router_id = f[3].to_string();
    let workspace_path = f[1].to_string();
    let fallback_project_path = std::path::Path::new(&workspace_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let project_path = f.get(19).unwrap_or(&"").to_string();
    let project_path = if project_path.is_empty() {
        fallback_project_path
    } else {
        project_path
    };
    let project = f.get(18).unwrap_or(&"").to_string();
    let project = if project.is_empty() {
        std::path::Path::new(&project_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Projects")
            .to_string()
    } else {
        project
    };
    let on = session::started(&agent, &router_id).unwrap_or_default();
    let next = session::pinned(&agent, &router_id).unwrap_or_default();
    let title = presentation_title(f[7], f.get(20).unwrap_or(&""));
    Some(Chat {
        project,
        project_path,
        repository_id: f.get(17).unwrap_or(&"").to_string(),
        workspace: f[0].to_string(),
        workspace_id: f.get(16).unwrap_or(&"").to_string(),
        path: workspace_path,
        session: f[2].to_string(),
        agent,
        status: f[5].to_string(),
        unread: f[6].parse().unwrap_or(0),
        title,
        context: f[8].parse().unwrap_or(0.0),
        context_tokens: f.get(9).and_then(|v| v.parse().ok()).unwrap_or(0),
        model: f.get(10).unwrap_or(&"").to_string(),
        permission: f.get(11).unwrap_or(&"").to_string(),
        effort: f.get(12).unwrap_or(&"").to_string(),
        personality: f.get(13).unwrap_or(&"").to_string(),
        fast: f.get(14).map(|v| *v == "1").unwrap_or(false),
        updated_at: f.get(15).unwrap_or(&"").to_string(),
        on,
        next,
    })
}

fn presentation_title(session: &str, workspace: &str) -> String {
    if !session.trim().is_empty()
        && !session.eq_ignore_ascii_case("new chat")
        && !session.eq_ignore_ascii_case("untitled")
    {
        return session.to_string();
    }
    let label = workspace.rsplit('/').next().unwrap_or(workspace).trim();
    if label.is_empty() {
        return session.to_string();
    }
    let mut words = label.replace(['-', '_'], " ");
    if let Some(first) = words.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    words
}

/// A chat with no pin follows its workspace, so the workspace's answer is what
/// its next process will take.
///
/// Deliberately not `store::effective_dir`. That goes through `resolve::decide`,
/// which is the router's decision and therefore writes: it can spend the account
/// chosen for a new workspace and record a route as a side effect. A listing
/// must not do that, and this one is part of each changed mobile snapshot.
///
/// So the read-only layers only, in the router's own order: an exact route, then
/// a repository binding, then a parent route or the default. Resolved once per
/// workspace rather than once per chat, which keeps large snapshots fast.
fn workspace_account(agent: &str, path: &str) -> String {
    let dir = std::path::Path::new(path);
    let found = routes::resolve(dir);
    if let Some(m) = &found {
        if m.exact {
            return m.profile.clone();
        }
    }
    if let Some(bound) = settings::repo_binding(agent, &store::repo_root(dir)) {
        if let Some(name) = store::profile_from_dir(&bound) {
            return name;
        }
    }
    found.map(|m| m.profile).unwrap_or_default()
}

fn fill_next(chats: &mut [Chat]) {
    let mut known: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    for chat in chats.iter_mut() {
        if !chat.next.is_empty() {
            continue;
        }
        let key = (chat.agent.clone(), chat.path.clone());
        let answer = known
            .entry(key)
            .or_insert_with(|| workspace_account(&chat.agent, &chat.path));
        chat.next.clone_from(answer);
    }
}

pub fn collect() -> Vec<Chat> {
    let detailed = places::rows(CHATS_DETAIL);
    let rows = if detailed.is_empty() {
        places::rows(CHATS)
    } else {
        detailed
    };
    let mut out: Vec<Chat> = rows.iter().filter_map(|l| parse(l)).collect();
    fill_next(&mut out);
    out
}

/// The same list as JSON, which is what anything drawing a screen wants.
pub fn as_json() -> Result<(), String> {
    println!("{}", json_string()?);
    Ok(())
}

pub fn json_string() -> Result<String, String> {
    store::ensure_root()?;
    let chats = collect();
    let pending = remote::counts();
    let wire: Vec<Wire> = chats
        .iter()
        .map(|c| Wire {
            project: &c.project,
            project_path: &c.project_path,
            repository_id: &c.repository_id,
            workspace: &c.workspace,
            workspace_id: &c.workspace_id,
            path: &c.path,
            session: &c.session,
            agent: &c.agent,
            status: &c.status,
            unread: c.unread,
            title: &c.title,
            context: c.context,
            context_tokens: c.context_tokens,
            model: &c.model,
            permission: &c.permission,
            effort: &c.effort,
            personality: &c.personality,
            fast: c.fast,
            updated_at: &c.updated_at,
            pending: pending.get(&c.session).copied().unwrap_or(0),
            on: &c.on,
            next: &c.next,
        })
        .collect();
    serde_json::to_string(&wire).map_err(|e| format!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::presentation_title;

    #[test]
    fn placeholder_titles_follow_conductors_workspace_label() {
        assert_eq!(
            presentation_title("New Chat", "feat/which-model-am-i"),
            "Which model am i"
        );
        assert_eq!(
            presentation_title("Untitled", "named_workspace"),
            "Named workspace"
        );
    }

    #[test]
    fn generated_chat_titles_win_over_workspace_labels() {
        assert_eq!(
            presentation_title("Fix model picker", "feat/which-model-am-i"),
            "Fix model picker"
        );
    }
}
