pub mod stable {
    use crate::common::{
        graph::BumpTree,
        workspace::{self, Workspace},
    };
    use semver::Version;

    pub fn exec(
        stable_workspace: &mut workspace::Workspace,
        package_name: &str,
        next_version: &Version,
        prerelease_workspace_option: Option<Workspace>,
        dry_run: bool,
    ) -> Result<(), String> {
        validate(stable_workspace, package_name, next_version)?;

        // Get the package
        let package = stable_workspace.packages.get(package_name).ok_or(format!(
            "Package {} does not exist in the workspace",
            package_name
        ))?;

        log::info!(
            "Building bump tree for {} to {}...",
            package_name,
            next_version
        );
        let mut bump_tree = BumpTree::new(stable_workspace, &prerelease_workspace_option);
        let root = bump_tree.build(package.clone(), next_version.clone());

        log::info!("Bump Tree");
        println!("{}", root.expect("root must be Some"));
        log::info!("{}", bump_tree.summary());

        if dry_run {
            log::info!("Dry-run: abort");
            return Ok(());
        }

        // Bump packages on the stable branch and commit the changes
        log::info!("Bumping packages on stable branch and committing changes...");
        stable_workspace.checkout_local_branch()?;
        for (package_name, b) in bump_tree.bumped.iter() {
            let package = stable_workspace
                .packages
                .get(package_name)
                .expect("Package must exist");

            package.borrow_mut().set_version(&b.next_version);
        }

        stable_workspace.update_lockfile()?;

        stable_workspace
            .stage_and_commit_all(format!("Bump {} to {}", package_name, next_version).as_str())?;

        // Bump packages on prerelease branch, if it exists
        if let Some(prerelease_workspace) = &prerelease_workspace_option {
            log::info!("Bumping packages on prerelease branch and committing changes...");
            prerelease_workspace.checkout_local_branch()?;

            let prerelease_branch_name =
                format!("propagate-{}-stable-bump-to-{}", package_name, next_version);
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
                    "Propagate stable bump of {} to {} to prerelease",
                    package_name, next_version
                )
                .as_str(),
            )?;
        }

        // Check back out to the original branch before exiting.
        stable_workspace.checkout_local_branch()?;

        log::info!("Workspace updated");

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
