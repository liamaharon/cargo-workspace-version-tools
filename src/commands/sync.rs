use crate::common::package::Package;
use crate::common::workspace::Workspace;
use crates_io_api::AsyncClient;
use semver::Version;

pub async fn exec(workspace: &mut Workspace) {
    // Instantiate the client.
    log::info!("Instantiating crates.io api client");
    let client = AsyncClient::new(
        "my-user-agent (liam@parity.io)",
        std::time::Duration::from_millis(1000),
    )
    .expect("Failed to create crates.io api client");

    // Check every manifest
    let total_files = workspace.packages.len();
    for (i, package) in workspace.packages.values_mut().enumerate() {
        let progress = format!("[{}/{}]", i, total_files);
        match sync_manifest(&client, &mut package.borrow_mut()).await {
            Ok(outcome) => match outcome {
                Outcome::AlreadyUpdated(v) => {
                    log::info!(
                        "{} ✅ {} already synced: {}",
                        progress,
                        package.borrow().name(),
                        v
                    );
                }
                Outcome::Updated(prev_version, new_version) => {
                    log::info!(
                        "{} 📝 Updated {} Cargo.toml to match crates.io ({} -> {})",
                        progress,
                        package.borrow().name(),
                        prev_version,
                        new_version
                    );
                }
                Outcome::PublishFalse => {
                    log::info!(
                        "{} 💤 {} publish = false, skipping",
                        progress,
                        package.borrow().name()
                    )
                }
            },
            Err(e) => log::error!(
                "{} ❌ Failed to check {} {}",
                progress,
                package.borrow().name(),
                e
            ),
        }
    }
}

async fn sync_manifest(client: &AsyncClient, package: &mut Package) -> Result<Outcome, String> {
    if !package.publish() {
        return Ok(Outcome::PublishFalse);
    }
    let package_version_before = package.version();
    let crates_version = package.crates_io_version(client).await?;
    // If versions dont match, update local to match crates.io
    if package_version_before != crates_version {
        package.set_version(&crates_version);
        return Ok(Outcome::Updated(package_version_before, crates_version));
    };

    Ok(Outcome::AlreadyUpdated(package_version_before))
}

pub enum Outcome {
    AlreadyUpdated(Version),
    Updated(Version, Version),
    PublishFalse,
}
