use crate::common::workspace::Workspace;

pub async fn exec(workspace: &mut Workspace) {
    for package in workspace.packages.values_mut() {
        let cur_version = package.borrow().version();

        // Remove any prerelease suffix
        let mut new_version = cur_version.clone();
        if !cur_version.pre.is_empty() {
            new_version.pre = semver::Prerelease::EMPTY;
        }

        // Bump to at least 0.1.0
        if new_version.major == 0 && new_version.minor == 0 {
            new_version.minor = 1;
            new_version.patch = 0;
        }

        if new_version != cur_version {
            log::info!(
                "📝 Updated {} version to allow compatible bumps ({} -> {})",
                package.borrow().name(),
                cur_version,
                new_version
            );
            package.borrow_mut().set_version(&new_version);
        }
    }
}
