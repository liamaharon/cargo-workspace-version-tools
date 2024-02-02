use clap::value_parser;
use env_logger::Env;
use semver::Version;
use std::path::PathBuf;

mod commands;
mod common;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cmd = clap::Command::new("Workspace Version Tools")
        .bin_name("workspace-version-tools")
        .subcommand_required(true)
        .args(&[
            clap::arg!(-w --workspace <PATH> "Workspace path").required(true).value_parser(value_parser!(String)),
        ])
        .subcommand(
            clap::command!("sync")
                .about("Sync local Cargo.toml files to match crates.io version")
        )
        .subcommand(
            clap::command!("bump")
                .subcommand_required(true)
                .about("Bump a package in the workspace")
                .args(&[
                    clap::arg!(-p --package <PACKAGE> "Package to bump").required(true),
                    clap::arg!(-v --version <VERSION> "New version (X.Y.Z)").required(true).value_parser(value_parser!(Version)),
                    clap::arg!(-d --dry_run <BOOL> "Whether to dry-run the change").default_value("false").value_parser(value_parser!(bool)),
                ])
                .subcommand(
                    clap::command!("stable")
                        .about("Bump a package on the stable branch")
                        .args(&[
                            clap::arg!(-p --prerelease <PRERELEASE_BRANCH> "Prerelease Git branch to keep consistent with this change"),
                        ])
                )
                .subcommand(
                    clap::command!("prerelease")
                        .about("Bump a package on a prerelease branch")
                        .args(&[
                            clap::arg!(-s --stable <STABLE_BRANCH> "Stable branch to cap the bump at"),
                        ])
                )
        );

    let matches = cmd.get_matches();
    let workspace_path = PathBuf::from(
        matches
            .get_one::<String>("workspace")
            .expect("--workspace is required"),
    );
    let mut workspace = common::workspace::Workspace::new(&workspace_path).unwrap();
    match matches.subcommand() {
        Some(("sync", _)) => {
            commands::sync::exec(&mut workspace).await;
        }
        Some(("bump", matches)) => {
            let package = matches
                .get_one::<String>("package")
                .expect("--package is required");
            let version = matches
                .get_one::<Version>("version")
                .expect("--version is required");
            let dry_run = matches
                .get_one::<bool>("dry_run")
                .expect("--dry_run is required");
            match matches.subcommand() {
                Some(("stable", matches)) => {
                    let prerelease_branch = matches.get_one::<String>("prerelease");
                    commands::bump::stable::exec(
                        &mut workspace,
                        &package,
                        &version,
                        prerelease_branch.map(|s| s.as_str()),
                        *dry_run,
                    )
                    .unwrap();
                }
                Some(("prerelease", _matches)) => {
                    todo!()
                }
                _ => unreachable!("clap should ensure we don't get here"),
            }
        }
        _ => unreachable!("clap should ensure we don't get here"),
    };
}
