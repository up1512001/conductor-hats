//! Copies the built panel and boot guard into OUT_DIR for `include_str!`.
//!
//! Node is needed to build the panel, but only by whoever builds a release: the
//! binary carries the compiled result. Missing input is a hard error, since a
//! binary that injects nothing is worse than one that fails to build.

use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");
    for name in ["account-ui.js", "boot-guard.js"] {
        let built = root.join("dist").join(name);
        println!("cargo:rerun-if-changed=dist/{name}");
        match std::fs::read(&built) {
            Ok(bytes) => {
                std::fs::write(out_dir.join(name), bytes).expect("writing the script into OUT_DIR")
            }
            Err(_) => panic!(
                "missing {}\n\
                 The injected scripts are generated, not committed. Build them first:\n\
                 \n    pnpm install && pnpm build\n",
                built.display()
            ),
        }
    }
}
