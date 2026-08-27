//! An opt-in log the panel can write to, so diagnosing it costs no re-patch.
//!
//! Every patch re-signs the copy, and a re-signed copy loses the keychain items
//! it stored, which signs it out of Conductor. Injecting a probe to find out why
//! the panel did something therefore cost a login every time. The panel can say
//! what it decided instead, when asked to.
//!
//! Off unless the flag file exists, and `hats debug status` says which it is,
//! because a tool that logs by default is a tool that logs something it should
//! not. It records what the panel resolved, never anything typed into a chat.

use std::io::Write;
use std::path::PathBuf;

fn flag() -> PathBuf {
    crate::paths::accounts_root().join("debug")
}

pub fn log_file() -> PathBuf {
    crate::paths::accounts_root().join("debug.log")
}

pub fn on() -> bool {
    flag().is_file()
}

pub fn run(rest: &[String]) -> Result<(), String> {
    match rest.first().map(String::as_str).unwrap_or("status") {
        "on" => {
            std::fs::write(flag(), b"").map_err(|e| format!("{e}"))?;
            println!("Panel logging on: {}", log_file().display());
            println!("It records what the panel resolved, nothing you type.");
            Ok(())
        }
        "off" => {
            let _ = std::fs::remove_file(flag());
            println!("Panel logging off.");
            Ok(())
        }
        "status" => {
            println!("{}", if on() { "on" } else { "off" });
            Ok(())
        }
        "read" => {
            let text = std::fs::read_to_string(log_file()).unwrap_or_default();
            print!("{text}");
            Ok(())
        }
        "clear" => {
            let _ = std::fs::remove_file(log_file());
            Ok(())
        }
        other => Err(format!(
            "usage: hats debug [on|off|status|read|clear], got '{other}'"
        )),
    }
}

/// Appends one line. Silent when logging is off, so the panel can call it freely.
pub fn write(rest: &[String]) -> Result<(), String> {
    if !on() {
        return Ok(());
    }
    let line = rest.join(" ");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file())
        .map_err(|e| format!("{e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("{e}"))
}

/// One line from inside the program, for the same log the panel writes to.
pub fn line(text: &str) {
    let _ = write(&[text.to_string()]);
}
