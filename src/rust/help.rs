//! The help text, kept out of the dispatch so both stay readable.

use crate::{DEV_APP, REAL_APP};

pub fn usage() {
    println!(
        "hats {}   one Claude or Codex account per Conductor workspace

Accounts
  add <profile> [agent]              create a profile and sign in to it
  login <profile> [agent]            sign in again
  logout <profile> [agent]           sign out, keep the profile
  remove <profile> [agent] [--force] sign out, delete the profile and its routes
  list [--mask]                      profiles, accounts and routes

Choosing one
  use <profile> [agent] [path]       point this workspace at a profile
  pin <profile> [agent] [session]    point one chat at a profile
  unpin [agent] [session]            let that chat follow the workspace
  bind <profile> [agent] [repo]      point a whole repository at one
  unbind [agent] [repo]              drop a repository binding
  assign <profile> [path]            the same as use, by path
  assign default <profile>           account for workspaces with no route
  unassign [path|default]            drop a route

Reporting
  status [path] [--mask]             what this workspace resolves to
  which [path] [agent]               the same, with every layer that fed in
  json [path]                        machine-readable, for the panel
  workspaces                         every workspace Conductor knows, name and path
  repos                              every repository, the same
  resolve <workspace-id>             its path, for the panel
  resolve-repo <repository-id>       the same for a repository
  check [path]                       one line, for an agent prompt
  mask <email>                       the masked form shown on screen
  doctor [path]                      check the setup end to end

The panel inside Conductor
  dev-app [--force]                  build an isolated Conductor copy
  patch [--app PATH] [--i-know]      inject the account panel into it
  patch --script FILE [--asset KEY] [--prepend]
                                     inject something else, for diagnosis
  revert [--app PATH]                restore the copy's original frontend
  repatch [--keep-app|--no-launch]   rebuild and re-inject after an update
  assets [--app PATH] [PATTERN]      list the frontend assets in a binary
  assets --dump PATTERN              print one asset decompressed, for diagnosis
  verify [--app PATH]                check a patched copy end to end
  debug [on|off|status|read|clear]   record what the panel resolved, for diagnosis
  reset-keychain [--app PATH]        forget what the copy stored, signing it out
  panel                              print the panel this binary carries
  guard                              print the boot guard this binary carries

Routing
  install                            turn routing on, add /account
  uninstall                          turn it off again
  session [path] [agent]             the chat currently live in a workspace
  sessions [clear]                   show or reset per-chat pins
  version

Patching rewrites a signed application, so it works on a copy by default:
  {DEV_APP}
Passing --i-know allows patching {REAL_APP}, which costs it notarization and
its keychain access.",
        env!("CARGO_PKG_VERSION")
    );
}
