//! The private CLI transport between the injected panel and the remote queue.

use crate::{
    auth, conductor_session, mobile_catalog, mobile_scope, mobile_service, origin, remote,
    remote_control, remote_create, remote_scan, source,
};

fn pair(workspace: &str, revoking: bool) -> Result<(), String> {
    let _origin = origin::configured()
        .ok_or("set the public HTTPS address before creating a pairing code")?;
    let source = source::for_workspace(workspace)?;
    let pairing = auth::mobile_pair(&source, revoking)?;
    let service = mobile_service::start(&source)?;
    println!(
        "{}",
        serde_json::json!({ "pairing": pairing, "service": service })
    );
    Ok(())
}

/// Commands that touch the queue, and so must see one Conductor copy only.
///
/// The pairing commands are deliberately absent: they decide which copy to bind
/// and have to be able to reach one that is not the bound one.
const CONFINED: [&str; 12] = [
    "catalog",
    "enqueue",
    "take",
    "purge",
    "claim",
    "confirm",
    "release",
    "pending",
    "next",
    "control-claim",
    "control-enqueue",
    "control-check",
];

pub fn run(rest: &[String]) -> Result<(), String> {
    let command = rest.first().map(String::as_str).unwrap_or("");
    if CONFINED.contains(&command)
        || command.starts_with("control-")
        || command.starts_with("create-")
    {
        mobile_scope::adopt()?;
    }
    match command {
        "catalog" => {
            let workspace = rest.get(1).map(String::as_str).unwrap_or("");
            let owner = source::for_workspace(workspace)?;
            let changed = mobile_catalog::publish(
                &owner,
                rest.get(2).map(String::as_str).unwrap_or(""),
            )?;
            println!("{{\"changed\":{changed}}}");
            Ok(())
        }
        "enqueue" => {
            let session = rest.get(1).map(String::as_str).unwrap_or("");
            let message = rest.get(2).map(String::as_str).unwrap_or("");
            println!(
                "{}",
                serde_json::to_string(&remote::enqueue(session, message)?)
                    .map_err(|e| format!("{e}"))?
            );
            Ok(())
        }
        "claim" => {
            let session = rest.get(1).map(String::as_str).unwrap_or("");
            println!(
                "{}",
                serde_json::to_string(&remote::claim(session)?).map_err(|e| format!("{e}"))?
            );
            Ok(())
        }
        "confirm" => {
            let delivered = remote::confirm(
                rest.get(1).map(String::as_str).unwrap_or(""),
                rest.get(2).map(String::as_str).unwrap_or(""),
                rest.get(3).map(String::as_str).unwrap_or(""),
            )?;
            println!("{{\"delivered\":{delivered}}}");
            Ok(())
        }
        "release" => remote::release(
            rest.get(1).map(String::as_str).unwrap_or(""),
            rest.get(2).map(String::as_str).unwrap_or(""),
            rest.get(3).map(String::as_str).unwrap_or(""),
        ),
        "pending" => {
            println!(
                "{}",
                remote::pending_json(rest.get(1).map(String::as_str).unwrap_or(""))?
            );
            Ok(())
        }
        "next" => {
            let workspace = rest.get(1).map(String::as_str).unwrap_or("");
            let mut queued = remote_scan::sessions();
            queued.extend(remote_control::sessions());
            queued.extend(remote_create::sessions());
            queued.sort();
            let sessions: Vec<String> = queued.into_iter().map(|(_, session)| session).collect();
            let route = conductor_session::first_in_workspace(workspace, &sessions);
            println!(
                "{}",
                serde_json::to_string(&route).map_err(|e| format!("{e}"))?
            );
            Ok(())
        }
        "purge" => {
            let dropped = remote::purge()?;
            let dropped = dropped + remote_create::purge();
            println!("{{\"dropped\":{dropped}}}");
            Ok(())
        }
        "take" => {
            let session = rest.get(1).map(String::as_str).unwrap_or("");
            let control = remote_control::claim(session)?;
            let create = match control {
                Some(_) => None,
                None => remote_create::claim(session)?,
            };
            let message = match (&control, &create) {
                (None, None) => remote::claim(session)?,
                _ => None,
            };
            println!(
                "{}",
                serde_json::json!({ "control": control, "create": create, "message": message })
            );
            Ok(())
        }
        "create-enqueue" => {
            let item = remote_create::enqueue(rest.get(1).map(String::as_str).unwrap_or(""))?;
            println!(
                "{}",
                serde_json::to_string(&item).map_err(|e| format!("{e}"))?
            );
            Ok(())
        }
        "create-complete" | "create-release" => {
            let raw = rest.get(1).map(String::as_str).unwrap_or("");
            let item = serde_json::from_str(raw).map_err(|_| "invalid new-chat claim")?;
            remote_create::finish(&item, rest[0] == "create-complete")
        }
        "create-check" => {
            let raw = rest.get(1).map(String::as_str).unwrap_or("");
            let item = serde_json::from_str(raw).map_err(|_| "invalid new-chat claim")?;
            let created = remote_create::check(&item)?;
            println!(
                "{}",
                serde_json::json!({ "applied": created.is_some(), "session": created })
            );
            Ok(())
        }
        "create-ack" => remote_create::ack(rest.get(1).map(String::as_str).unwrap_or("")),
        "control-claim" => {
            let session = rest.get(1).map(String::as_str).unwrap_or("");
            println!(
                "{}",
                serde_json::to_string(&remote_control::claim(session)?).map_err(|e| format!("{e}"))?
            );
            Ok(())
        }
        "control-enqueue" => {
            let item = remote_control::enqueue(
                rest.get(1).map(String::as_str).unwrap_or(""),
                rest.get(2).map(String::as_str).unwrap_or(""),
                rest.get(3).map(String::as_str).unwrap_or(""),
                rest.get(4).map(String::as_str).unwrap_or(""),
            )?;
            println!(
                "{}",
                serde_json::to_string(&item).map_err(|e| format!("{e}"))?
            );
            Ok(())
        }
        "control-complete" | "control-release" => {
            let raw = rest.get(1).map(String::as_str).unwrap_or("");
            let item = serde_json::from_str(raw).map_err(|_| "invalid control claim")?;
            remote_control::finish(&item, rest[0] == "control-complete")
        }
        "control-check" => {
            let raw = rest.get(1).map(String::as_str).unwrap_or("");
            let item = serde_json::from_str(raw).map_err(|_| "invalid control claim")?;
            println!("{{\"applied\":{}}}", remote_control::applied(&item));
            Ok(())
        }
        "control-ack" => remote_control::ack(
            rest.get(1).map(String::as_str).unwrap_or(""),
            rest.get(2).map(String::as_str).unwrap_or(""),
        ),
        "mobile-status" => {
            let workspace = rest.get(1).map(String::as_str).unwrap_or("");
            let requested = if workspace.is_empty() {
                None
            } else {
                Some(source::for_workspace(workspace)?)
            };
            let pairing = match requested.as_ref() {
                Some(source) => auth::active_pairing_for(source)?,
                None => auth::active_pairing()?,
            };
            println!(
                "{}",
                serde_json::json!({
                    "origin": origin::configured().unwrap_or_default(),
                    "pairing": pairing,
                    "service": requested.as_ref().map(mobile_service::status_for).unwrap_or_else(mobile_service::status),
                })
            );
            Ok(())
        }
        "mobile-origin" => {
            let origin = origin::save(rest.get(1).map(String::as_str).unwrap_or(""))?;
            let requested = rest
                .get(2)
                .map(|workspace| source::for_workspace(workspace))
                .transpose()?;
            println!(
                "{}",
                serde_json::json!({
                    "origin": origin,
                    "service": requested.as_ref().map(mobile_service::status_for).unwrap_or_else(mobile_service::status),
                })
            );
            Ok(())
        }
        "mobile-pair" => pair(rest.get(1).map(String::as_str).unwrap_or(""), false),
        "mobile-revoke" => pair(rest.get(1).map(String::as_str).unwrap_or(""), true),
        "mobile-stop" => {
            let requested = source::for_workspace(rest.get(1).map(String::as_str).unwrap_or(""))?;
            let service = if mobile_scope::matches(&requested) {
                let stopped = mobile_service::stop_for(&requested)?;
                auth::invalidate_all()?;
                stopped
            } else {
                mobile_service::status_for(&requested)
            };
            println!("{}", serde_json::to_string(&service).map_err(|e| format!("{e}"))?);
            Ok(())
        }
        _ => Err("usage: hats remote <catalog|take|purge|claim|confirm|release|pending|next|mobile-status|mobile-origin|mobile-pair|mobile-revoke|mobile-stop> ...".into()),
    }
}
