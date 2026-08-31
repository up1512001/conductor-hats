//! Read-only proof that Conductor applied a queued run setting.

use crate::{conductor_session, remote_control::Control};

pub fn valid(setting: &str, value: &str) -> bool {
    matches!(setting, "model" | "effort" | "permission" | "fast")
        && !value.trim().is_empty()
        && value.len() <= 100
        && !value.contains(['\0', '\n', '\r'])
}

fn current_matches(control: &Control) -> bool {
    let Some(current) = conductor_session::setting(&control.session, &control.setting) else {
        return false;
    };
    match control.setting.as_str() {
        "fast" => (current == "1") == (control.value == "on"),
        "model" | "effort" | "permission" => current == control.value,
        _ => false,
    }
}

pub fn applied_session(control: &Control) -> Option<String> {
    if current_matches(control) {
        return Some(control.session.clone());
    }
    (control.setting == "model" && control.marker > 0)
        .then(|| {
            conductor_session::created_model_since(&control.session, control.marker, &control.value)
        })
        .flatten()
}

pub fn applied(control: &Control) -> bool {
    applied_session(control).is_some()
}
