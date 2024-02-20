use super::common::get_mock_workspaces;
use crate::common::bump_tree::tree::BumpTree;
use crate::common::bump_tree::{instruction::BumpInstruction, tree::ReleaseChannel};
use semver::Version;
use std::str::FromStr;

#[derive(Clone)]
struct VersionChangeAssertion {
    package_name: String,
    initial_stable_version: Option<Version>,
    initial_prerelease_version: Option<Version>,
    expected_stable_version: Option<Version>,
    expected_prerelease_version: Option<Version>,
}

/// Asserts that with the given bump instruction, the following package assertions follow after
/// being processed by a bump tree. If None is passed, we don't check.
fn run_bump_tree_assertion(
    raw_bump_instruction: &str,
    version_change_assertions: Vec<VersionChangeAssertion>,
    release_channel: ReleaseChannel,
) {
    let (stable_workspace, prerelease_workspace) = get_mock_workspaces();

    // Set initial versions
    for version_change_assertion in version_change_assertions.clone() {
        version_change_assertion.initial_stable_version.map(|v| {
            let name = version_change_assertion.package_name.clone();
            let p = stable_workspace.packages.get(&name).unwrap();
            p.borrow_mut().set_version(&v);
        });
        version_change_assertion
            .initial_prerelease_version
            .map(|v| {
                let name = version_change_assertion.package_name.clone();
                let p = prerelease_workspace.packages.get(&name).unwrap();
                p.borrow_mut().set_version(&v);
            });
    }

    // Build bump tree
    let root_nodes = match BumpInstruction::from_str(
        &stable_workspace,
        &prerelease_workspace,
        &raw_bump_instruction,
        release_channel,
    )
    .unwrap()
    {
        Some(i) => vec![i],
        None => vec![],
    };
    let tree = BumpTree::new(
        &stable_workspace,
        &prerelease_workspace,
        root_nodes,
        release_channel,
    );

    // Assert expected results
    for version_change_assertion in version_change_assertions {
        match version_change_assertion.expected_stable_version {
            Some(v) => {
                let name = version_change_assertion.package_name.clone();
                let p = stable_workspace.packages.get(&name).unwrap();
                assert_eq!(
                    tree.highest_stable.get(&name).unwrap().stable,
                    Some(BumpInstruction {
                        package: p.clone(),
                        next_version: v,
                    })
                );
            }
            None => {
                assert_eq!(
                    tree.highest_stable
                        .get(&version_change_assertion.package_name),
                    None
                )
            }
        }

        match version_change_assertion.expected_prerelease_version {
            Some(v) => {
                let name = version_change_assertion.package_name.clone();
                let p = prerelease_workspace.packages.get(&name).unwrap();
                assert_eq!(
                    tree.highest_prerelease.get(&name).unwrap().prerelease,
                    Some(BumpInstruction {
                        package: p.clone(),
                        next_version: v,
                    })
                );
            }
            None => {
                assert_eq!(
                    tree.highest_prerelease
                        .get(&version_change_assertion.package_name),
                    None
                )
            }
        };
    }
}

pub mod prerelease {
    use super::*;

    pub mod major {
        use super::*;

        #[test]
        fn success_when_matches_stable() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.0").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_major() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_minor() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn success_when_already_ahead_by_patch() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.1-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_no_stable() {
            run_bump_tree_assertion(
                "prerelease-only-1-0-0 major",
                vec![VersionChangeAssertion {
                    package_name: "prerelease-only-1-0-0".to_owned(),
                    initial_stable_version: None,
                    initial_prerelease_version: None,
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }
    }

    pub mod minor {
        use super::*;

        #[test]
        fn success_when_matches_stable() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.0").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_major() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_minor() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn success_when_already_ahead_by_patch() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.1-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_no_stable() {
            run_bump_tree_assertion(
                "prerelease-only-1-0-0 minor",
                vec![VersionChangeAssertion {
                    package_name: "prerelease-only-1-0-0".to_owned(),
                    initial_stable_version: None,
                    initial_prerelease_version: None,
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }
    }

    pub mod patch {
        use super::*;

        #[test]
        fn success_when_matches_stable() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.0").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: Some(Version::from_str("1.0.1-alpha").unwrap()),
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_major() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_minor() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_already_ahead_by_patch() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.1-alpha").unwrap()),
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }

        #[test]
        fn noop_when_no_stable() {
            run_bump_tree_assertion(
                "prerelease-only-1-0-0 patch",
                vec![VersionChangeAssertion {
                    package_name: "prerelease-only-1-0-0".to_owned(),
                    initial_stable_version: None,
                    initial_prerelease_version: None,
                    expected_stable_version: None,
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Prerelease,
            );
        }
    }
}

pub mod stable {
    use super::*;

    pub mod major {
        use super::*;

        #[test]
        fn causes_minor_ahead_prerelease_to_leapfrog() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                    expected_stable_version: Some(Version::from_str("2.0.0").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("3.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_major_ahead_prerelease_to_leapfrog() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                    expected_stable_version: Some(Version::from_str("2.0.0").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("3.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_equal_prerelease_to_leapfrog() {
            run_bump_tree_assertion(
                "a major",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.0").unwrap()),
                    expected_stable_version: Some(Version::from_str("2.0.0").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("3.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_dependents_to_major_bump() {
            run_bump_tree_assertion(
                "a major",
                vec![
                    VersionChangeAssertion {
                        package_name: "a".to_owned(),
                        initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                        expected_stable_version: Some(Version::from_str("2.0.0").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("3.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                    VersionChangeAssertion {
                        package_name: "b".to_owned(),
                        initial_stable_version: Some(Version::from_str("0.1.0").unwrap()),
                        expected_stable_version: Some(Version::from_str("0.2.0").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("1.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                    VersionChangeAssertion {
                        package_name: "c".to_owned(),
                        initial_stable_version: Some(Version::from_str("2.3.1").unwrap()),
                        expected_stable_version: Some(Version::from_str("3.0.0").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("4.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                ],
                ReleaseChannel::Stable,
            );
        }
    }

    pub mod minor {
        use super::*;

        #[test]
        fn causes_equal_prerelease_to_leapfrog() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.0").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.1.0").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_minor_ahead_prerelease_to_major_bump() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.1.0").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.1.0").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn has_no_impact_on_major_ahead_prerelease() {
            run_bump_tree_assertion(
                "a minor",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.1.0").unwrap()),
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_dependents_to_patch() {
            run_bump_tree_assertion(
                "a minor",
                vec![
                    VersionChangeAssertion {
                        package_name: "a".to_owned(),
                        initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                        expected_stable_version: Some(Version::from_str("1.1.0").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                    VersionChangeAssertion {
                        package_name: "b".to_owned(),
                        initial_stable_version: Some(Version::from_str("0.1.0").unwrap()),
                        expected_stable_version: Some(Version::from_str("0.1.1").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("1.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                    VersionChangeAssertion {
                        package_name: "c".to_owned(),
                        initial_stable_version: Some(Version::from_str("2.3.1").unwrap()),
                        expected_stable_version: Some(Version::from_str("2.3.2").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("3.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                ],
                ReleaseChannel::Stable,
            );
        }
    }

    pub mod patch {
        use super::*;

        #[test]
        fn causes_equal_prerelease_to_leapfrog() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.0").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.0.1").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("1.0.2-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_patch_ahead_prerelease_to_bump() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.0.1-alpha").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.0.1").unwrap()),
                    expected_prerelease_version: Some(Version::from_str("1.0.2-alpha").unwrap()),
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn has_no_impact_on_minor_ahead_prerelease() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("1.1.0-alpha").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.0.1").unwrap()),
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn has_no_impact_on_major_ahead_prerelease() {
            run_bump_tree_assertion(
                "a patch",
                vec![VersionChangeAssertion {
                    package_name: "a".to_owned(),
                    initial_stable_version: Some(Version::from_str("1.0.0").unwrap()),
                    initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                    expected_stable_version: Some(Version::from_str("1.0.1").unwrap()),
                    expected_prerelease_version: None,
                }],
                ReleaseChannel::Stable,
            );
        }

        #[test]
        fn causes_dependents_to_patch() {
            run_bump_tree_assertion(
                "a patch",
                vec![
                    VersionChangeAssertion {
                        package_name: "a".to_owned(),
                        initial_stable_version: Some(Version::from_str("1.0.1").unwrap()),
                        expected_stable_version: Some(Version::from_str("1.0.2").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("2.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                    VersionChangeAssertion {
                        package_name: "b".to_owned(),
                        initial_stable_version: Some(Version::from_str("0.1.0").unwrap()),
                        expected_stable_version: Some(Version::from_str("0.1.1").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("1.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                    VersionChangeAssertion {
                        package_name: "c".to_owned(),
                        initial_stable_version: Some(Version::from_str("2.3.1").unwrap()),
                        expected_stable_version: Some(Version::from_str("2.3.2").unwrap()),
                        // set this to major version above so it will noop; we're not trying to
                        // test it here
                        initial_prerelease_version: Some(Version::from_str("3.0.0-alpha").unwrap()),
                        expected_prerelease_version: None,
                    },
                ],
                ReleaseChannel::Stable,
            );
        }
    }
}
