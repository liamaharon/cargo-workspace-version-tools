use super::package::Package;
use crate::common::graph::find_direct_dependents;
use cargo_metadata::MetadataCommand;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
};

/// An in-memory representation of the workspace members
pub struct Workspace {
    /// Members of the workspace
    pub packages: HashMap<String, Rc<RefCell<Package>>>,
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
        let workspace_package_map = cargo_metadata_members
            .iter()
            .map(|p| {
                Package::new(&p, &workspace_member_names)
                    .map_err(|e| format!("Failed to load package at {:?}: {}", p, e))
            })
            .fold(HashMap::new(), |mut acc, package_result| {
                match package_result {
                    Ok(package) => {
                        log::debug!("Loaded package {}", package);
                        acc.insert(package.name(), Rc::new(RefCell::new(package)));
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
                        .borrow()
                        .direct_workspace_dependencies()
                        .iter()
                        .map(|dep| dep.clone())
                        .collect::<HashSet<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();

        for (name, package) in workspace_package_map.iter() {
            let direct_dependents = find_direct_dependents(name, &workspace_deps_map);
            package
                .borrow_mut()
                .set_direct_dependents(direct_dependents);
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
