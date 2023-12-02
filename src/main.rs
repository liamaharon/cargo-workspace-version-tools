mod commands;

#[tokio::main]
async fn main() {
    let cmd = clap::Command::new("Workspace Version Tools")
        .bin_name("workspace-version-tools")
        .subcommand_required(true)
        .subcommand(
            clap::command!("sync-local")
                .about("Sync local Cargo.toml files to match crates.io version")
                .arg(
                    clap::arg!(--"workspace-path")
                        .help("The path to the workspace to sync")
                        .value_parser(clap::value_parser!(std::path::PathBuf))
                        .required(true),
                ),
        );

    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("sync-local", matches)) => {
            let workspace_path = matches
                .get_one::<std::path::PathBuf>("workspace-path")
                .unwrap();
            commands::sync::exec(workspace_path.into()).await;
        }
        _ => unreachable!("clap should ensure we don't get here"),
    };
}
