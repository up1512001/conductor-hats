//! Copies the built panel into OUT_DIR for `include_str!`.
//!
//! Node is needed to build the panel, but only by whoever builds a release: the
//! binary carries the compiled result. Missing input is a hard error, since a
//! binary that injects nothing is worse than one that fails to build.

use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let panel = root.join("dist/account-ui.js");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("account-ui.js");

    println!("cargo:rerun-if-changed=dist/account-ui.js");
    println!("cargo:rerun-if-changed=build.rs");

    match std::fs::read(&panel) {
        Ok(bytes) => std::fs::write(&out, bytes).expect("writing the panel into OUT_DIR"),
        Err(_) => panic!(
            "missing {}\n\
             The panel is generated, not committed. Build it first:\n\
             \n    pnpm install && pnpm build\n",
            panel.display()
        ),
    }
}
