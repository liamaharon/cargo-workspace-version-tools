use std::{fmt::Display, str::FromStr};

use semver::Version;

#[derive(Clone)]
pub struct PackageChange {
    pub name: String,
    pub version: Version,
}

impl FromStr for PackageChange {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, ' ').collect();
        let name = parts[0].to_string();
        let version = Version::parse(parts[1])
            .map_err(|e| format!("Failed to parse version {}: {}", parts[1], e))?;
        Ok(PackageChange { name, version })
    }
}

impl Display for PackageChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}

pub mod stable {
    use super::*;
    use crate::common::{
        colors::{BLUE, GREEN},
        graph::BumpTree,
        workspace::{self, Workspace},
    };

    pub fn exec(
        stable_workspace: &mut workspace::Workspace,
        package_changes: Vec<&PackageChange>,
        prerelease_workspace_option: Option<Workspace>,
        dry_run: bool,
    ) -> Result<(), String> {
        for entry in package_changes.iter() {
            validate(stable_workspace, &entry.name, &entry.version)?;
        }

        log::info!("⏳Building bump tree...");
        let root_packages = package_changes
            .iter()
            .map(|entry| {
                let package = stable_workspace
                    .packages
                    .get(&entry.name)
                    .expect("Package {} does not exist in the workspace")
                    .clone();
                let next_version = entry.version.clone();
                (package, next_version)
            })
            .collect::<Vec<_>>();
        let bump_tree = BumpTree::new(
            stable_workspace,
            &prerelease_workspace_option,
            root_packages,
        );

        println!("{}", bump_tree);

        if dry_run {
            log::info!("Dry-run: aborting");
            return Ok(());
        }

        // Bump packages on the stable branch and commit the changes
        log::info!("{}---------------------------------------", BLUE);
        log::info!("{}Applying version bumps to stable branch", BLUE);
        log::info!("{}---------------------------------------", BLUE);
        stable_workspace.checkout_local_branch()?;
        for (package_name, b) in bump_tree.bumped.iter() {
            let package = stable_workspace
                .packages
                .get(package_name)
                .expect("Package must exist");

            package.borrow_mut().set_version(&b.next_version);
        }

        stable_workspace.update_lockfile()?;

        let changes_string_vec = &package_changes
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>();

        stable_workspace.stage_and_commit_all(
            format!("Apply bumps {}", changes_string_vec.join(", ")).as_str(),
        )?;

        // Bump packages on prerelease branch, if it exists
        if let Some(prerelease_workspace) = &prerelease_workspace_option {
            log::info!("{}-------------------------------------------", BLUE);
            log::info!("{}Applying version bumps to prerelease branch", BLUE);
            log::info!("{}-------------------------------------------", BLUE);
            prerelease_workspace.checkout_local_branch()?;

            let prerelease_branch_name = format!(
                "propagate-{}-bump-to-prerelease",
                changes_string_vec.join("_")
            );
            prerelease_workspace
                .create_and_checkout_branch(prerelease_branch_name.as_str())
                .map_err(|e| e.to_string())?;

            for (package_name, b) in bump_tree.bumped.iter() {
                match prerelease_workspace.packages.get(package_name) {
                    Some(prerelease_package) => {
                        prerelease_package.borrow_mut().set_version(
                            &b.next_prerelease_version
                                .clone()
                                .expect("New version in bump tree for a package that doesn't exist in workspace!"),
                        );
                    }
                    None => {
                        log::info!(
                            "No package found in the prerelease workspace for package {}, skipping",
                            package_name
                        );
                    }
                }
            }

            prerelease_workspace.update_lockfile()?;
            prerelease_workspace.stage_and_commit_all(
                format!(
                    "Propagate stable {} bump to prerelease",
                    changes_string_vec.join(", ")
                )
                .as_str(),
            )?;

            log::info!("❗ Don't forget to run `git push {} {}` and open a PR to update the prerelease branch!", stable_workspace.remote_name, prerelease_branch_name);
        }

        // Check back out to the original branch before exiting.
        log::info!(
            "{}-------------------------------------------------------------",
            GREEN
        );
        log::info!(
            "{}Done! Checking back out to the original branch before exiting",
            GREEN
        );
        log::info!(
            "{}-------------------------------------------------------------",
            GREEN
        );
        stable_workspace.checkout_local_branch()?;

        Ok(())
    }

    fn validate(
        workspace: &mut workspace::Workspace,
        package: &str,
        version: &Version,
    ) -> Result<(), String> {
        // Version must not be pre-release
        if !version.pre.is_empty() {
            return Err(format!(
                "Version provided {} is a pre-release version, stable releases must be non-pre-release",
                version
            ));
        }

        // Version must be gt current version
        let current_version = workspace
            .packages
            .get(package)
            .expect("Package must exist")
            .borrow()
            .version();
        if version <= &current_version {
            return Err(format!(
                "Version provided {} is not greater than the current package version {}",
                version, current_version
            ));
        }

        Ok(())
    }
}
