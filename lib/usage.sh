#!/bin/bash
# The help text.
#
# Sourced by bin/conductor-acct. Not executable on its own.

usage() {
    cat <<EOF
conductor-acct $CONDUCTOR_ACCT_VERSION - one Claude/Codex account per Conductor workspace

  setup                            guided first run
  add <profile> [claude|codex]     create a profile and sign in to it
  use <profile> [agent] [path]     point this workspace at a profile
  status [path] [--mask]           what this workspace resolves to, in two lines
  which [path] [agent]             the same, with every layer that fed into it
  list [--mask]                    profiles, accounts and routes
  mask <email>                     the masked form shown on screen

  login <profile> [agent]          re-run sign in for a profile
  logout <profile> [agent]         sign out, keep the profile
  remove <profile> [agent]         sign out, delete the profile and its routes

  bind <profile> [agent] [repo]    bind a whole repository to a profile
  unbind [agent] [repo]            drop a repository binding
  assign default <profile>         account for workspaces with no route
  unassign [path|default]          drop a route

  ask on|off|status [repo]         ask in the first chat of a new workspace
  check [path]                     ACCOUNT/NEEDS_ACCOUNT, for the ask prompt
  json [path] [agent]              accounts and current selection, as JSON
  resolve <workspace-id>           workspace id to directory, for the UI panel
  resolve-repo <repository-id>     repository id to its root directory
  workspaces                       name and path of every live workspace
  repos                            name and path of every repository

  login-start <profile> [agent]    begin sign-in, print the authorisation URL
  login-code <profile> <code>      hand the pasted code to a pending sign-in
  login-status <profile> [agent]   idle | pending | ok <email> | error <msg>
  login-cancel <profile>           abandon a pending sign-in

  install                          turn the router on, add /account
  uninstall                        turn it off again
  update                           pull the checkout and reinstall
  sessions [clear]                 show or reset per-session pins
  doctor                           check the setup end to end

Env overrides: CONDUCTOR_ACCOUNT forces a profile for one spawn,
CONDUCTOR_ACCOUNTS_ROOT moves the state directory.
EOF
}
