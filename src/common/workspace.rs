use super::{
    git::{checkout_local_branch, create_and_checkout_branch, stage_and_commit_all_changes},
    package::Package,
};
use crate::common::{
    git::{do_fast_forward, do_fetch, get_current_branch_name, is_working_tree_clean},
    package::find_direct_dependents,
};
use cargo_metadata::MetadataCommand;
use git2::Repository;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Command,
    rc::Rc,
};

/// An in-memory representation of the workspace members
pub struct Workspace {
    /// Members of the workspace
    pub packages: HashMap<String, Rc<RefCell<Package>>>,
    /// Workspace path
    pub path: PathBuf,
    /// Git branch
    pub branch_name: String,
    /// Git remote
    pub remote_name: String,
}

impl Workspace {
    pub fn new(
        workspace_path: &PathBuf,
        branch_name: Option<&str>,
        remote_name: &str,
    ) -> Result<Self, String> {
        let repo = Repository::open(&workspace_path)
            .map_err(|e| format!("Failed to open repository at {:?}: {}", &workspace_path, e))?;

        let cargo_toml_path = workspace_path.join("Cargo.toml");
        let branch_name = match branch_name {
            Some(branch_name) => branch_name.to_owned(),
            None => get_current_branch_name(&repo)
                .expect("Failed to get current branch name")
                .to_owned(),
        };

        log::info!(
            "⏳Building workspace for path {:?} branch {}...",
            &cargo_toml_path,
            &branch_name
        );

        if !is_working_tree_clean(&repo) {
            return Err("Workspace is not clean. Please commit or stash your changes.".to_owned());
        }

        log::info!(
            "Pulling latest changes from remote '{} {}'",
            &remote_name,
            &branch_name
        );
        let mut remote = repo
            .find_remote(&remote_name)
            .map_err(|e| format!("{}", e))?;
        let fetch_commit =
            do_fetch(&repo, &[&branch_name], &mut remote).map_err(|e| format!("{}", e))?;
        do_fast_forward(&repo, &branch_name, fetch_commit).map_err(|e| format!("{}", e))?;

        // Create the Packages
        let metadata = MetadataCommand::new()
            .manifest_path(&cargo_toml_path)
            .exec()
            .map_err(|e| format!("Failed to load workspace at {:?}: {}", &cargo_toml_path, e))?;

        let cargo_metadata_members = metadata.workspace_packages();
        let workspace_member_names = cargo_metadata_members
            .iter()
            .map(|p| p.name.clone())
            .collect::<HashSet<_>>();
        let workspace_package_map = cargo_metadata_members
            .iter()
            .map(|p| {
                Package::new(&p, &workspace_member_names, branch_name.as_str())
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
        let workspace_deps_string_set = workspace_package_map
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
            let direct_dependents = find_direct_dependents(name, &workspace_deps_string_set)
                .into_iter()
                .map(|s| {
                    (
                        s.clone(),
                        workspace_package_map
                            .get(&s)
                            .expect("just got it bro")
                            .clone(),
                    )
                })
                .collect::<HashMap<_, _>>();
            package
                .borrow_mut()
                .set_direct_dependents(direct_dependents);
        }

        let w = Workspace {
            packages: workspace_package_map,
            path: workspace_path.clone(),
            branch_name,
            remote_name: remote_name.to_owned(),
        };

        log::info!("Workspace built ✅");

        Ok(w)
    }

    pub fn stage_and_commit_all(&self, message: &str) -> Result<(), String> {
        let repo = self.open_repository();
        stage_and_commit_all_changes(&repo, &self.branch_name, message)
            .map_err(|e| format!("{}", e))?;
        Ok(())
    }

    pub fn open_repository(&self) -> Repository {
        Repository::open(&self.path).expect("Failed to open repository")
    }

    /// Hack to quickly update the Cargo.lock based only on workspace changes
    pub fn update_lockfile(&self) -> Result<(), String> {
        log::info!("⏳Updating branch {} Cargo.lock...", &self.branch_name);
        let output = Command::new("cargo")
            .arg("metadata")
            .arg("--manifest-path")
            .arg(&self.path.join("Cargo.toml"))
            .current_dir(&self.path)
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        if output.status.success() {
            log::info!("Done ✅");
        } else {
            log::warn!("Issue updating Cargo.lock");
        }

        Ok(())
    }

    pub fn create_and_checkout_branch(&self, branch_name: &str) -> Result<(), String> {
        let repo = self.open_repository();
        create_and_checkout_branch(&repo, &self.remote_name, branch_name).map_err(|e| e.to_string())
    }

    pub fn checkout_local_branch(&self) -> Result<(), String> {
        let repo = self.open_repository();
        checkout_local_branch(&repo, &self.branch_name).map_err(|e| e.to_string())?;
        log::info!("Checked out branch {}", &self.branch_name);
        Ok(())
    }
}
