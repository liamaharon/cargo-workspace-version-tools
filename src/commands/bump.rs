pub mod stable {
    use crate::common::{graph::BumpTree, version_extension::VersionExtension, workspace};
    use semver::Version;

    pub fn exec(
        workspace: &mut workspace::Workspace,
        package_name: &str,
        next_version: &Version,
        prerelease_branch: Option<&str>,
        dry_run: bool,
    ) -> Result<(), String> {
        validate(workspace, package_name, next_version)?;

        // Get the package
        let package = workspace
            .packages
            .get(package_name)
            .expect("Package must exist");

        let mut bump_tree = BumpTree::new(workspace);
        let root = bump_tree.build(package.clone(), next_version.clone());

        log::info!("\nBump Tree\n{}", root);

        Ok(())
    }

    fn validate(
        workspace: &mut workspace::Workspace,
        package: &str,
        version: &Version,
    ) -> Result<(), String> {
        // Package must exist in the workspace
        if !workspace.packages.contains_key(package) {
            return Err(format!(
                "Package {} does not exist in the workspace",
                package
            ));
        }

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
