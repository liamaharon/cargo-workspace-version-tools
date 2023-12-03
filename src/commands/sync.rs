use crates_io_api::AsyncClient;
use std::path::Path;
use std::{fs, path::PathBuf};
use toml_edit::{value, Document};

pub async fn exec(workspace_path: &PathBuf) {
    // Instantiate the client.
    log::info!("Instantiating crates.io api client");
    let client = AsyncClient::new(
        "my-user-agent (liam.aharon@hotmail.com)",
        std::time::Duration::from_millis(1000),
    )
    .expect("Failed to create crates.io api client");

    // Find all Cargo.toml files in the workspace
    let cargo_files =
        find_manifest_paths(workspace_path, true).expect("Failed to find manifest paths");
    log::info!("Found {} Cargo.toml files", cargo_files.len());

    // Check every manifest
    for file_path in cargo_files {
        match check_manifest(&client, &file_path).await {
            Ok(outcome) => match outcome {
                Outcome::AlreadyUpdated(v) => {
                    log::info!("✅ Already up-to-date: {}", v);
                }
                Outcome::Updated(file_path, new_version) => {
                    log::info!(
                        "📝 Updated Cargo.toml version to match crates.io ({} -> {})",
                        file_path,
                        new_version
                    );
                }
                Outcome::PublishFalse => {
                    log::info!("💤 'publish = false' set, skipping")
                }
            },
            Err(e) => log::error!("❌ Failed to check {} {}", file_path, e),
        }
    }
}

async fn check_manifest(client: &AsyncClient, file_path: &str) -> Result<Outcome, Error> {
    // Read the Cargo.toml file
    let content = fs::read_to_string(&file_path)?;
    let mut doc = content.parse::<Document>()?;

    // Get package
    let package = match doc.get_mut("package").and_then(|p| p.as_table_mut()) {
        Some(package) => package,
        None => return Err(Error::InvalidPackageTable),
    };

    // Get package name
    let name = match package.get("name").and_then(|n| n.as_str()) {
        Some(name) => name,
        None => return Err(Error::InvalidPackageName),
    };

    // Check if publish = false
    if let Some(publish) = package.get("publish").and_then(|p| p.as_bool()) {
        if !publish {
            return Ok(Outcome::PublishFalse);
        }
    }

    // Get version specified in the crate
    let local_version = match package.get("version").and_then(|v| v.as_str()) {
        Some(v) => v.to_owned(),
        None => return Err(Error::InvalidPackageVersion),
    };

    // Get crates.io version
    let crates_io_version = client.get_crate(name).await?.crate_data.max_version;

    // If versions dont match, update local to match crates.io
    if local_version != crates_io_version {
        package["version"] = value(crates_io_version.clone());
        // Write the changes back to the Cargo.toml file and print a blank line
        fs::write(file_path, doc.to_string())?;
        return Ok(Outcome::Updated(
            local_version,
            crates_io_version.to_owned(),
        ));
    };

    Ok(Outcome::AlreadyUpdated(local_version.to_owned()))
}

fn find_manifest_paths<P: AsRef<Path>>(
    path: P,
    is_root: bool,
) -> Result<Vec<String>, std::io::Error> {
    let mut manifest_paths = Vec::new();
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let path = entry.path();

        // Skip the 'target' directory only if it's in the root
        if path.is_dir() {
            if is_root && path.file_name() == Some(std::ffi::OsStr::new("target")) {
                continue;
            }

            // Recursively search in directories, marking them as non-root
            let mut sub_files = find_manifest_paths(&path, false)?;
            manifest_paths.append(&mut sub_files);
        } else if path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml")) {
            manifest_paths.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(manifest_paths)
}

pub enum Error {
    Io(std::io::Error),
    TomlEdit(toml_edit::TomlError),
    CratesIoApi(crates_io_api::Error),
    InvalidPackageTable,
    InvalidPackageName,
    InvalidPackageVersion,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::TomlEdit(e) => write!(f, "TOML error: {}", e),
            Error::CratesIoApi(e) => write!(f, "Crates.io API error: {}", e),
            Error::InvalidPackageTable => write!(f, "Invalid package table"),
            Error::InvalidPackageName => write!(f, "Invalid package name"),
            Error::InvalidPackageVersion => write!(f, "Invalid package version"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<toml_edit::TomlError> for Error {
    fn from(e: toml_edit::TomlError) -> Self {
        Error::TomlEdit(e)
    }
}

impl From<crates_io_api::Error> for Error {
    fn from(e: crates_io_api::Error) -> Self {
        Error::CratesIoApi(e)
    }
}

pub enum Outcome {
    AlreadyUpdated(String),
    Updated(String, String),
    PublishFalse,
}
