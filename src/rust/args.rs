//! Command line flags shared by the patching commands.
//!
//! Defaults come from the environment so a sandbox can point every path
//! somewhere harmless without passing flags through each call.

use std::path::PathBuf;

use crate::{DEV_APP, DEV_ID, REAL_APP};

pub(crate) struct Args {
    pub dump: bool,
    pub prepend: bool,
    pub asset: Option<String>,
    pub scripts: Vec<(Option<String>, bool, PathBuf)>,
    pub app: PathBuf,
    pub src: PathBuf,
    pub id: String,
    pub i_know: bool,
    pub force: bool,
    pub rebuild: bool,
    pub launch: bool,
    pub pattern: Option<String>,
}

pub(crate) fn parse(rest: &[String]) -> Args {
    let mut args = Args {
        app: env_path("CONDUCTOR_DEV_APP", DEV_APP),
        src: env_path("CONDUCTOR_APP", REAL_APP),
        id: std::env::var("CONDUCTOR_DEV_ID").unwrap_or_else(|_| DEV_ID.into()),
        dump: false,
        prepend: false,
        asset: None,
        scripts: Vec::new(),
        i_know: false,
        force: false,
        rebuild: true,
        launch: true,
        pattern: None,
    };
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dump" => args.dump = true,
            "--prepend" => args.prepend = true,
            "--script" => {
                if let Some(v) = it.next() {
                    args.scripts
                        .push((args.asset.take(), args.prepend, PathBuf::from(v)));
                    args.prepend = false;
                }
            }
            "--asset" => {
                if let Some(v) = it.next() {
                    args.asset = Some(v.to_string());
                }
            }
            "--app" => {
                if let Some(v) = it.next() {
                    args.app = PathBuf::from(v);
                }
            }
            "--src" => {
                if let Some(v) = it.next() {
                    args.src = PathBuf::from(v);
                }
            }
            "--i-know" => args.i_know = true,
            "--force" => args.force = true,
            "--keep-app" => args.rebuild = false,
            "--no-launch" => args.launch = false,
            other => args.pattern = Some(other.to_string()),
        }
    }
    args
}

fn env_path(key: &str, fallback: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}
