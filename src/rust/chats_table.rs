//! The terminal table for `hats chats`, kept apart from collecting the list.

use crate::{chats, mask, profile, store};

fn shown(agent: &str, name: &str, masked: bool) -> String {
    if name.is_empty() {
        return "-".into();
    }
    match (masked, profile::label(agent, name)) {
        (true, Some(email)) if !email.is_empty() => mask::email(&email),
        _ => name.to_string(),
    }
}

pub fn run(masked: bool) -> Result<(), String> {
    store::ensure_root()?;
    let chats = chats::collect();
    if chats.is_empty() {
        println!("No chats. Conductor's database is unreadable, or it has none open.");
        return Ok(());
    }

    println!(
        "{:<20} {:<8} {:<9} {:>6}  {:<10} {:<10} TITLE",
        "WORKSPACE", "AGENT", "STATUS", "CTX", "ON", "NEXT"
    );
    for c in &chats {
        let unread = if c.unread > 0 {
            format!(" ({} unread)", c.unread)
        } else {
            String::new()
        };
        println!(
            "{:<20} {:<8} {:<9} {:>5.0}%  {:<10} {:<10} {}{}",
            c.workspace,
            c.agent,
            c.status,
            c.context,
            shown(&c.agent, &c.on, masked),
            shown(&c.agent, &c.next, masked),
            c.title,
            unread
        );
    }

    let moving = chats
        .iter()
        .filter(|c| !c.on.is_empty() && !c.next.is_empty() && c.on != c.next)
        .count();
    if moving > 0 {
        println!();
        println!(
            "{moving} chat(s) will change account when reopened. A running \
             conversation keeps the account it started on."
        );
    }
    Ok(())
}
