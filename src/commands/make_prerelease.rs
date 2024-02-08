use std::str::FromStr;

use semver::Version;

use crate::common::workspace::Workspace;

pub async fn exec(workspace: &mut Workspace) -> Result<(), String> {
    for package in workspace.packages.values_mut() {
        let cur_version = package.borrow().version();

        // Version should not already be prerelease
        if !cur_version.pre.is_empty() {
            return Err(format!(
                "Package {} already has a prerelease version. Check your branch.",
                package.borrow().name()
            ));
        }

        let new_version = Version::from_str(format!("{}-alpha.1", cur_version).as_str())
            .expect(format!("Failed to append prerelease suffix to {}", cur_version).as_str());
        log::info!(
            "📝 Updated {} version to prerelease ({} -> {})",
            package.borrow().name(),
            cur_version,
            &new_version
        );
        package.borrow_mut().set_version(&new_version);
    }

    Ok(())
}
