use std::path::PathBuf;

use env_logger::Env;

mod commands;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cmd = clap::Command::new("Workspace Version Tools")
        .bin_name("workspace-version-tools")
        .subcommand_required(true)
        .subcommand(
            clap::command!("sync")
                .about("Sync local Cargo.toml files to match crates.io version")
                .args(&[
                    clap::arg!(-p --path <PATH> "Path to the workspace to sync").required(true)
                ]),
        );

    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("sync", matches)) => {
            let workspace_path =
                PathBuf::from(matches.get_one::<String>("path").expect("Path is required"));
            commands::sync::exec(&workspace_path.into()).await;
        }
        _ => unreachable!("clap should ensure we don't get here"),
    };
}
