use std::{path::Path, str::FromStr};

use semver::Version;

use crate::common::{
    bump_tree::instruction::BumpInstruction, version_extension::BumpType, workspace::Workspace,
};

fn get_mock_workspaces() -> (Workspace, Workspace) {
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

    (
        Workspace::new_test_workspace(&mocks_root.join("stable_workspace")).unwrap(),
        Workspace::new_test_workspace(&mocks_root.join("prerelease_workspace")).unwrap(),
    )
}

pub mod bump_type {
    use super::*;

    #[test]
    fn handles_versions_with_major() {
        let (stable_workspace, _) = get_mock_workspaces();

        let package = stable_workspace.packages.get("a1-0-0").unwrap();
        let version = package.borrow().version().clone();
        assert_eq!(version, Version::from_str("1.0.0").unwrap());

        let major_bump_instruction = BumpInstruction {
            package: package.clone(),
            next_version: Version::from_str("2.0.0").unwrap(),
        };
        assert_eq!(major_bump_instruction.bump_type(), BumpType::Major);

        let minor_bump_instruction = BumpInstruction {
            package: package.clone(),
            next_version: Version::from_str("1.1.0").unwrap(),
        };
        assert_eq!(minor_bump_instruction.bump_type(), BumpType::Minor);

        let patch_bump_instruction = BumpInstruction {
            package: package.clone(),
            next_version: Version::from_str("1.0.1").unwrap(),
        };
        assert_eq!(patch_bump_instruction.bump_type(), BumpType::Patch);
    }

    #[test]
    fn handles_versions_without_major() {
        let (stable_workspace, _) = get_mock_workspaces();

        let package = stable_workspace.packages.get("a0-1-0").unwrap();
        let version = package.borrow().version().clone();
        assert_eq!(version, Version::from_str("0.1.0").unwrap());

        let major_bump_instruction = BumpInstruction {
            package: package.clone(),
            next_version: Version::from_str("1.0.0").unwrap(),
        };
        assert_eq!(major_bump_instruction.bump_type(), BumpType::Major);

        let major_bump_instruction = BumpInstruction {
            package: package.clone(),
            next_version: Version::from_str("0.2.0").unwrap(),
        };
        assert_eq!(major_bump_instruction.bump_type(), BumpType::Major);

        let patch_bump_instruction = BumpInstruction {
            package: package.clone(),
            next_version: Version::from_str("0.1.1").unwrap(),
        };
        assert_eq!(patch_bump_instruction.bump_type(), BumpType::Patch);
    }
}

pub mod from_str {
    use crate::common::{
        bump_tree::tree::ReleaseChannel,
        version_extension::{EndUserInitiated, VersionExtension},
    };

    use super::*;

    #[test]
    fn prerelease_only_package_doesnt_need_bump() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-only-1-0-0 patch",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );
    }

    #[test]
    fn prerelease_already_major_bumped_package_doesnt_need_bump() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-only-1-0-0 patch",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );
    }

    #[test]
    fn bump_prerelease_only_package_on_stable_channel_fails() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();

        assert!(matches!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-only-1-0-0 patch",
                ReleaseChannel::Stable,
            ),
            Err(_)
        ));
    }

    #[test]
    fn bump_stable_only_package_on_prerelease_channel_fails() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();

        assert!(matches!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "stable-only-1-0-0 patch",
                ReleaseChannel::Prerelease,
            ),
            Err(_)
        ));
    }

    #[test]
    fn bump_stable_patch_works() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();
        let package = stable_workspace.packages.get("stable-only-1-0-0").unwrap();

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "stable-only-1-0-0 patch",
                ReleaseChannel::Stable,
            ),
            Ok(Some(BumpInstruction {
                package: package.clone(),
                next_version: Version::from_str("1.0.1").unwrap()
            }))
        );
    }

    #[test]
    fn bump_stable_minor_works() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();
        let package = stable_workspace.packages.get("stable-only-1-0-0").unwrap();

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "stable-only-1-0-0 minor",
                ReleaseChannel::Stable,
            ),
            Ok(Some(BumpInstruction {
                package: package.clone(),
                next_version: Version::from_str("1.1.0").unwrap()
            }))
        );
    }

    #[test]
    fn bump_stable_major_works() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();
        let package = stable_workspace.packages.get("stable-only-1-0-0").unwrap();

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "stable-only-1-0-0 major",
                ReleaseChannel::Stable,
            ),
            Ok(Some(BumpInstruction {
                package: package.clone(),
                next_version: Version::from_str("2.0.0").unwrap()
            }))
        );
    }

    #[test]
    fn bump_prerelease_major_already_bumped_is_noop() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-major major",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-major minor",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-major patch",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );
    }

    #[test]
    fn bump_prerelease_minor_already_bumped_works() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();
        let package = prerelease_workspace
            .packages
            .get("prerelease-ahead-minor")
            .unwrap();

        let expected_next_version = package
            .borrow()
            .version()
            .bump(BumpType::Major, EndUserInitiated::Yes);

        // Major bump works
        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-minor major",
                ReleaseChannel::Prerelease,
            ),
            Ok(Some(BumpInstruction {
                package: package.clone(),
                next_version: expected_next_version
            }))
        );

        // minor is noop (already bumped)
        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-minor minor",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );

        // minor is noop (already bumped)
        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-minor patch",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );
    }

    #[test]
    fn bump_prerelease_patch_already_bumped_works() {
        let (stable_workspace, prerelease_workspace) = get_mock_workspaces();
        let package = prerelease_workspace
            .packages
            .get("prerelease-ahead-patch")
            .unwrap();

        let expected_next_version = package
            .borrow()
            .version()
            .bump(BumpType::Major, EndUserInitiated::Yes);

        // Major bump works
        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-patch major",
                ReleaseChannel::Prerelease,
            ),
            Ok(Some(BumpInstruction {
                package: package.clone(),
                next_version: expected_next_version
            }))
        );

        // minor bump works
        let expected_next_version = package
            .borrow()
            .version()
            .bump(BumpType::Minor, EndUserInitiated::Yes);

        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-patch minor",
                ReleaseChannel::Prerelease,
            ),
            Ok(Some(BumpInstruction {
                package: package.clone(),
                next_version: expected_next_version
            }))
        );

        // minor is noop (already bumped)
        assert_eq!(
            BumpInstruction::from_str(
                &stable_workspace,
                &prerelease_workspace,
                "prerelease-ahead-patch patch",
                ReleaseChannel::Prerelease,
            ),
            Ok(None)
        );
    }
}
