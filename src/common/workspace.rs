use super::package::Package;
use crate::common::graph::{find_dependencies, find_dependents};
use cargo_metadata::MetadataCommand;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

/// An in-memory representation of the workspace members
pub struct Workspace {
    /// Members of the workspace
    pub packages: HashMap<String, Package>,
}

impl Workspace {
    pub fn new(workspace_path: &PathBuf) -> Result<Self, String> {
        let cargo_toml_path = workspace_path.join("Cargo.toml");
        let metadata = MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .map_err(|e| format!("Failed to load workspace at {:?}: {}", &cargo_toml_path, e))?;

        // Create the Packages
        let cargo_metadata_members = metadata.workspace_packages();
        let workspace_member_names = cargo_metadata_members
            .iter()
            .map(|p| p.name.clone())
            .collect::<HashSet<_>>();
        let mut workspace_package_map = cargo_metadata_members
            .iter()
            .map(|p| {
                Package::new(&p, &workspace_member_names)
                    .map_err(|e| format!("Failed to load package at {:?}: {}", p, e))
            })
            .fold(HashMap::new(), |mut acc, package_result| {
                match package_result {
                    Ok(package) => {
                        log::debug!("Loaded package {}", package);
                        acc.insert(package.name(), package);
                    }
                    Err(e) => log::error!("{}", e),
                };
                acc
            });

        // Compute and set the dependencies and dependents
        let start = std::time::Instant::now();
        let workspace_deps_map = workspace_package_map
            .iter()
            .map(|(name, package)| {
                (
                    name.clone(),
                    package
                        .direct_workspace_dependencies
                        .iter()
                        .map(|dep| dep.clone())
                        .collect::<HashSet<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();

        for (name, package) in workspace_package_map.iter_mut() {
            package.set_all_workspace_dependencies(find_dependencies(&name, &workspace_deps_map));
            package.set_all_workspace_dependents(find_dependents(&name, &workspace_deps_map));
        }
        log::debug!(
            "Computed dependencies and dependents in {}ms",
            start.elapsed().as_millis()
        );

        Ok(Workspace {
            packages: workspace_package_map,
        })
    }
}
