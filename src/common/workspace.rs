use cargo_metadata::{Metadata, MetadataCommand};
use std::{collections::HashMap, path::PathBuf};

use super::package::Package;

/// An in-memory representation of the workspace members
pub struct Workspace {
    /// Members of the workspace
    pub packages: HashMap<String, Package>,
    /// Raw cargo metadata
    metadata: Metadata,
}

impl Workspace {
    pub fn new(workspace_path: &PathBuf) -> Result<Self, String> {
        let cargo_toml_path = workspace_path.join("Cargo.toml");
        let metadata = MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .map_err(|e| format!("Failed to load workspace at {:?}: {}", &cargo_toml_path, e))?;

        let members = metadata.workspace_packages();

        // Build packages
        let packages = members
            .iter()
            .map(|p| {
                Package::new(&p.manifest_path.clone().into())
                    .map_err(|e| format!("Failed to load package at {:?}: {}", p, e))
            })
            .collect::<Vec<Result<Package, String>>>();

        let mut package_map = HashMap::new();
        for package in packages {
            match package {
                Ok(package) => {
                    log::debug!("Loaded package {}", package);
                    package_map.insert(package.name(), package);
                }
                Err(e) => log::error!("{}", e),
            }
        }

        Ok(Workspace {
            packages: package_map,
            metadata,
        })
    }
}
