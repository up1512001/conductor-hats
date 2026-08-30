//! The mobile access drill-down and its security-significant presentation.

mod common;

fn source(path: &str) -> String {
    std::fs::read_to_string(common::repo().join(path)).unwrap_or_default()
}

fn must(body: &str, needles: &[&str]) {
    for needle in needles {
        assert!(body.contains(needle), "missing {needle:?}");
    }
}

#[test]
fn mobile_access_is_a_real_panel_view() {
    let root = source("src/panel/views/root.ts");
    let triggers = source("src/panel/triggers.ts");
    let controller = source("src/panel/controller.ts");
    must(
        &triggers,
        &[
            "Mobile access",
            "cma-mobile-btn",
            "refreshMobileTrigger",
            "is-connected",
        ],
    );
    must(
        &controller,
        &[
            "mobileView(host)",
            "anchor.id === \"cma-mobile-btn\"",
            "mobile ? \"mobile\" : \"root\"",
        ],
    );
    assert!(
        !root.contains("Mobile access"),
        "mobile access is nested in the account panel"
    );
}

#[test]
fn the_pairing_view_has_the_complete_t3_style_share_flow() {
    let mobile = source("src/panel/views/mobile.ts");
    must(
        &mobile,
        &[
            "Create pairing code",
            "Expires in ${Math.ceil(seconds / 60)} min",
            "Copy link",
            "Revoke paired phones",
            "Revoke mobile access?",
            "Change public address",
            "mobileCommand(\"mobile-pair\")",
            "mobileCommand(\"mobile-revoke\")",
            "fresh 64-character path",
            "Starting secure service…",
            "phone${service.connections === 1",
            "Stop mobile access",
            "mobileCommand(\"mobile-stop\")",
        ],
    );
}

#[test]
fn the_open_conductor_app_owns_the_pairing_and_phone_screen() {
    let mobile = source("src/panel/views/mobile.ts");
    let service = source("src/rust/mobile_service.rs");
    let state = source("src/rust/mobile_state.rs");
    must(
        &mobile,
        &[
            "fromToolbar().workspace",
            "q(workspaceScope)",
            "service.source",
        ],
    );
    must(
        &service,
        &[".env(\"CONDUCTOR_DB\", source.database())", "status_for"],
    );
    must(&state, &["source: String", "crate::source::active()"]);
}

#[test]
fn model_account_and_thinking_controls_live_in_the_chat_composer() {
    let page = source("src/mobile/index.html");
    let app = source("src/mobile/app.ts");
    let render = source("src/mobile/render.ts");
    must(&page, &["id=\"composer-tools\""]);
    must(
        &render,
        &[
            "composerControls",
            "controlMenu",
            "data-control=",
            "\"account\"",
            "\"model\"",
            "\"effort\"",
        ],
    );
    must(&app, &["openControl", "chooseControl", "updateSend()"]);
    assert!(!render.contains("run-settings"));
}

#[test]
fn model_and_thinking_choices_match_conductor() {
    let catalog = source("src/panel/model_catalog.ts");
    let state = source("src/rust/mobile_state.rs");
    let render = source("src/mobile/render.ts");
    must(
        &catalog,
        &[
            "visibleBuiltInModelIds",
            "conductorCatalog()",
            "visibleTitles(titles)",
            "remote catalog ",
        ],
    );
    must(
        &render,
        &[
            "[\"low\", \"medium\", \"high\", \"xhigh\", \"max\"]",
            "Extra high",
            "modelLabel(value)",
            "models?.[agent]",
            "Claude Code",
            "Codex",
        ],
    );
    must(&state, &["apply_titles", "catalog.titles"]);
    assert!(!state.contains("gpt-5.2-codex"));
    assert!(!state.contains("claude-opus-4-8-v1"));
}

#[test]
fn reported_header_and_toolbar_spacing_is_explicit() {
    let mobile = source("src/mobile/styles.css");
    let head = mobile
        .lines()
        .find(|line| line.starts_with(".head {"))
        .expect("the mobile header rule");
    assert!(
        !head.contains("border-bottom"),
        "the header divider returned"
    );
    must(
        &source("src/panel/styles/_triggers.scss"),
        &["margin-right: 4px", "#cma-toolbar-btn", "margin-right: 8px"],
    );
}

#[test]
fn the_qr_is_local_high_contrast_and_does_not_print_the_secret() {
    let qr = source("src/panel/qr.ts");
    let mobile = source("src/panel/views/mobile.ts");
    must(
        &qr,
        &[
            "qrFactory(0, \"M\")",
            "background.setAttribute(\"fill\", \"#fff\")",
            "modules.setAttribute(\"fill\", \"#000\")",
            "shape-rendering",
        ],
    );
    assert!(mobile.contains("shortPath + \"#token=••••••••\""));
    assert!(!mobile.contains("el(\"code\", null, pairing.url)"));
}

#[test]
fn public_address_setup_requires_https_and_pairing_starts_loopback() {
    let mobile = source("src/panel/views/mobile.ts");
    must(
        &mobile,
        &[
            "https://conductor.example.com",
            "remote mobile-origin",
            "listener stays on loopback",
            "starts the protected loopback service automatically",
        ],
    );
    assert!(!mobile.contains("hats serve"));
    assert!(!mobile.contains("cloudflared"));

    let service = source("src/rust/mobile_service.rs");
    must(
        &service,
        &[
            "127.0.0.1:8787",
            ".process_group(0)",
            ".stdout(Stdio::null())",
            "if status().running",
        ],
    );
}

#[test]
fn new_chat_reconnect_and_send_failures_have_recoverable_state() {
    let app = source("src/mobile/app.ts");
    let create = source("src/mobile/create.ts");
    let echo = source("src/mobile/echo.ts");
    let socket = source("src/rust/mobile_socket.rs");
    must(
        &create,
        &[
            "type: \"new-chat\"",
            "type: \"create-ack\"",
            "created.result",
            "created.error",
        ],
    );
    must(
        &app,
        &["creation.resume()", "transport.send({ type: \"subscribe\""],
    );
    must(
        &echo,
        &[
            "before: number",
            "occurrences(snapshot, text)",
            "reject(request",
        ],
    );
    must(
        &socket,
        &["\"request\": request", "accepted-new-chat", "control-ack"],
    );
}

#[test]
fn cross_provider_models_follow_conductors_new_chat() {
    let body = source("src/mobile/control.ts");
    must(
        &body,
        &[
            "item.state === \"done\"",
            "control-ack",
            "Model opened in a new chat on your Mac",
            "return moved.result",
        ],
    );
}
