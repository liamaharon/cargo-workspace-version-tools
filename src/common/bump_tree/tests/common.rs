use crate::common::workspace::Workspace;
use fs_extra::dir::{self, CopyOptions};
use std::path::Path;
use tempdir::TempDir;

/// Create copies of the mock workspaces which can be safely modified in tests.
pub(crate) fn get_mock_workspaces() -> (Workspace, Workspace) {
    let mocks_root = Path::new(&file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("mocks");

    let temp_dir_path = TempDir::new("workspace-version-tools-test")
        .unwrap()
        .into_path();
    let temp_stable_workspace_path = temp_dir_path.join("stable_workspace");
    let temp_prerelease_workspace_path = temp_dir_path.join("prerelease_workspace");
    let mut options = CopyOptions::new();
    options.overwrite = true;
    dir::copy(
        &mocks_root.join("stable_workspace"),
        &temp_dir_path,
        &options,
    )
    .expect("failed to copy workspace");

    dir::copy(
        &mocks_root.join("prerelease_workspace"),
        &temp_dir_path,
        &options,
    )
    .expect("failed to copy workspace");

    (
        Workspace::new_test_workspace(temp_stable_workspace_path).unwrap(),
        Workspace::new_test_workspace(temp_prerelease_workspace_path).unwrap(),
    )
}
