//! Cargo Workspace Version Tools is a CLI to ease the complexity of managing package versions in large Cargo
//! workspaces containing inter-dependent packages.
//!
//! It is opinionated in assuming the workspace wishes packages to follow SemVer version.
//!
//! In addition to supporting a single release channel, it also supports dual stable/prerelease
//! release channels where the prerelease channel is periodically merged into stable.

use clap::{value_parser, ArgAction};
use common::workspace::Workspace;
use env_logger::Env;
use std::{borrow::BorrowMut, path::PathBuf};

mod commands;
mod common;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    match run().await {
        Ok(_) => {}
        Err(e) => {
            log::error!("{}", e);
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<(), String> {
    let cmd = clap::Command::new("Workspace Version Tools")
        .bin_name("workspace-version-tools")
        .subcommand_required(true)
        .args(&[
            clap::arg!(-w --workspace <PATH> "Workspace path").required(true).value_parser(value_parser!(String)),
            clap::arg!(-r --"git-remote" [REMOTE] "Git remote").value_parser(value_parser!(String)).default_value("origin"),
        ])
        .subcommand(
            clap::command!("sync")
                .about("Sync local Cargo.toml files to match crates.io version")
        )
        .subcommand(
            clap::command!("make-at-least-stable")
                .about("Make local Cargo.toml versions support compatible bumps by removing prerelease suffixes and bumping to at least 0.1.0.")
        )
        .subcommand(
            clap::command!("bump")
                .subcommand_required(true)
                .about("Bump a package in the workspace")
                .args(&[
                    clap::arg!(-b --"bump-instruction" <BUMP_INSTRUCTION> "Package and type of bump to make to it, e.g. \"pallet-balances minor\". Supports being passed multiple times to bump multiple packages at once.")
                        .required(true)
                        .action(ArgAction::Append)
                        .value_parser(value_parser!(String)),
                    clap::arg!(-d --"dry-run" [BOOL] "Whether to dry-run the change")
                        .default_value("false")
                        .default_missing_value("true")
                        .value_parser(value_parser!(bool))
                ])
                .subcommand(
                    clap::command!("stable")
                        .about("Bump a package on the stable branch")
                        .args(&[
                            clap::arg!(-p --"prerelease-branch" <PRERELEASE_BRANCH> "Also update a prerelease branch to keep the version distance the same after this change"),
                        ])
                )
                .subcommand(
                    clap::command!("prerelease")
                        .about("Bump a package on the prerelease branch")
                        .args(&[
                            clap::arg!(-s --"stable-branch" <STABLE_BRANCH> "Stable branch to cap the bump at"),
                        ])
                )
        );

    let matches = cmd.get_matches();
    let workspace_path = PathBuf::from(
        matches
            .get_one::<String>("workspace")
            .expect("--workspace is required"),
    );
    let remote_name = matches
        .get_one::<String>("git-remote")
        .expect("--git-remote is required");
    let mut workspace = Workspace::new(workspace_path.clone(), None, remote_name)?;

    match matches.subcommand() {
        Some(("sync", _)) => {
            commands::sync::exec(&mut workspace).await;
            Ok(())
        }
        Some(("make-at-least-stable", _)) => {
            commands::make_at_least_stable::exec(&mut workspace).await;
            Ok(())
        }
        Some(("bump", matches)) => {
            let bump_instructions = matches
                .get_many::<String>("bump-instruction")
                .expect("--bump-instruction is required")
                .collect::<Vec<_>>();
            let dry_run = matches
                .get_one::<bool>("dry-run")
                .expect("--dry-run is required");
            match matches.subcommand() {
                Some(("stable", matches)) => {
                    let prerelease_workspace = matches
                        .get_one::<String>("prerelease-branch")
                        .map(|b| Workspace::new(workspace_path, Some(b.as_str()), remote_name));

                    let prerelease_workspace = match prerelease_workspace {
                        Some(Ok(prerelease_workspace)) => Some(prerelease_workspace),
                        Some(Err(e)) => return Err(e),
                        None => None,
                    };

                    commands::bump::exec_stable(
                        &mut workspace,
                        prerelease_workspace
                            .expect("Currently must also update prerelease branch")
                            .borrow_mut(),
                        bump_instructions
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>(),
                        *dry_run,
                    )
                }
                Some(("prerelease", matches)) => {
                    let stable_workspace = matches
                        .get_one::<String>("stable-branch")
                        .map(|b| Workspace::new(workspace_path, Some(b.as_str()), remote_name));

                    let stable_workspace = match stable_workspace {
                        Some(Ok(w)) => Some(w),
                        Some(Err(e)) => return Err(e),
                        None => None,
                    };

                    commands::bump::exec_prerelease(
                        stable_workspace
                            .expect("Currently must also update stable branch")
                            .borrow_mut(),
                        &mut workspace,
                        bump_instructions
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>(),
                        *dry_run,
                    )
                }
                _ => unreachable!("clap should ensure we don't get here"),
            }
        }
        _ => unreachable!("clap should ensure we don't get here"),
    }
}
