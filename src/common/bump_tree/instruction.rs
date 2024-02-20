use super::tree::{BumpTree, ReleaseChannel};
use crate::common::version_extension::{EndUserInitiated, VersionExtension};
use crate::common::workspace::Workspace;
use crate::common::{package::Package, version_extension::BumpType};
use semver::Version;
use std::{
    cell::RefCell,
    collections::HashSet,
    fmt::{self, Display, Formatter},
    rc::Rc,
};

#[derive(Debug, Clone)]
pub struct BumpInstruction {
    pub package: Rc<RefCell<Package>>,
    pub next_version: Version,
}

impl BumpInstruction {
    pub fn bump_type(&self) -> BumpType {
        let cur_version = self.package.borrow().version();
        if self.next_version.major > cur_version.major
            || (self.next_version.major == 0
                && cur_version.major == 0
                && self.next_version.minor > cur_version.minor)
        {
            BumpType::Major
        } else if self.next_version.minor > cur_version.minor {
            BumpType::Minor
        } else {
            BumpType::Patch
        }
    }

    pub fn from_str(
        stable_workspace: &Workspace,
        prerelease_workspace: &Workspace,
        s: &str,
        release_channel: ReleaseChannel,
    ) -> Result<Option<BumpInstruction>, String> {
        let parts: Vec<&str> = s.splitn(2, ' ').collect();
        let name = parts[0].to_string();
        let semver_part = parts
            .get(1)
            .map(|b| BumpType::from_str(b))
            .unwrap_or_else(|| Err(format!("Invalid Bump Instruction: '{}'", s).to_string()))?;

        let stable_package = match (stable_workspace.packages.get(&name), &release_channel) {
            // If we have a package, we can proceed
            (Some(p), _) => p,
            // Doesn't make sense to try to bump a stable package doesn't exist
            (None, ReleaseChannel::Stable) => {
                return Err(format!(
                    "Package {} not found on branch {}",
                    name, prerelease_workspace.branch_name
                ))
            }
            (None, ReleaseChannel::Prerelease) => {
                // If there's no stable package for a prerelease bump, there's no need to do anything.
                log::info!(
                    "🤙 Prerelease package {} isn't yet on the stable release channel, so there is no need to bump it",
                    name,
                );
                return Ok(None);
            }
        };
        let cur_stable_version = stable_package.borrow().version();

        match (release_channel, prerelease_workspace.packages.get(&name)) {
            // Stable is easy, just bump the version.
            (ReleaseChannel::Stable, _) => Ok(Some(BumpInstruction {
                package: stable_package.clone(),
                next_version: cur_stable_version.bump(semver_part, EndUserInitiated::Yes),
            })),
            // Handle no prerelease package when user asking to bump it
            (ReleaseChannel::Prerelease, None) => Err(format!(
                "Package {} not found on branch {}",
                name, prerelease_workspace.branch_name
            )),
            // Prerelease, need to determine what the next version should be relative to the
            // existing stable package.
            (ReleaseChannel::Prerelease, Some(prerelease_package)) => {
                let cur_prerelease_version = prerelease_package.borrow().version();
                match semver_part {
                    BumpType::Major => {
                        // Ignore minor bump if already ahead on major
                        if cur_prerelease_version.major > cur_stable_version.major {
                            log::info!(
                                "🤙 Prerelease package {} (v{}) is already a major bump ahead of stable (v{}), so there is no need to major bump it",
                                name,
                                cur_prerelease_version,
                                cur_stable_version
                           );
                            return Ok(None);
                        }

                        // Need to bump to stable major+1
                        Ok(Some(BumpInstruction {
                            package: prerelease_package.clone(),
                            next_version: cur_stable_version
                                .bump(BumpType::Major, EndUserInitiated::Yes)
                                .with_prerelease(),
                        }))
                    }
                    BumpType::Minor => {
                        // Ignore minor bump if already ahead on major or minor
                        if cur_prerelease_version.major > cur_stable_version.major
                            || cur_prerelease_version.minor > cur_stable_version.minor
                        {
                            log::info!(
                                "🤙 Prerelease package {} (v{}) is already a minor bump ahead of stable (v{}), so there is no need to minor bump it",
                                name,
                                cur_prerelease_version,
                                cur_stable_version
                           );
                            return Ok(None);
                        }

                        // Need to bump to stable minor+1
                        Ok(Some(BumpInstruction {
                            package: prerelease_package.clone(),
                            next_version: cur_stable_version
                                .bump(BumpType::Minor, EndUserInitiated::Yes)
                                .with_prerelease(),
                        }))
                    }
                    BumpType::Patch => {
                        // Ignore patch bump if already ahead on major or minor or patch
                        if cur_prerelease_version.major > cur_stable_version.major
                            || cur_prerelease_version.minor > cur_stable_version.minor
                            || cur_prerelease_version.patch > cur_stable_version.patch
                        {
                            log::info!(
                                "🤙 Prerelease package {} (v{}) is already a patch bump ahead of stable (v{}), so there is no need to patch bump it",
                                name,
                                cur_prerelease_version,
                                cur_stable_version
                           );
                            return Ok(None);
                        }

                        // Need to bump to stable patch+1
                        Ok(Some(BumpInstruction {
                            package: prerelease_package.clone(),
                            next_version: cur_stable_version
                                .bump(BumpType::Patch, EndUserInitiated::Yes)
                                .with_prerelease(),
                        }))
                    }
                }
            }
        }
    }
}

impl PartialEq for BumpInstruction {
    fn eq(&self, other: &Self) -> bool {
        self.package.borrow().name() == other.package.borrow().name()
            && self.package.borrow().branch == other.package.borrow().branch
            && self.bump_type() == other.bump_type()
    }
}

/// Prerelease bump type is influenced by the parent and the next stable bump type.
/// It also requires a stable package to exist for this child, otherwise the prerelease
/// isn't being bumped in relation to anything.
pub fn compute_prerelease_bump_instruction(
    prerelease_package: Option<&Rc<RefCell<Package>>>,
    stable_package: Option<&Rc<RefCell<Package>>>,
    stable_bump_instruction: Option<&BumpInstruction>,
    prerelease_parent_bump_instruction: Option<&BumpInstruction>,
) -> Option<BumpInstruction> {
    // If there's no prerelease package, there's nothing to bump
    let prerelease_package = match prerelease_package {
        Some(p) => p,
        None => return None,
    };
    let cur_prerelease_version = prerelease_package.borrow().version();

    // If there's no stable package, then there's no reason to bump the prerelease version because
    // its current version is already ready to release to stable.
    let stable_package = match stable_package {
        Some(p) => p,
        None => return None,
    };
    let cur_stable_version = stable_package.borrow().version();

    // First candidate for the bump type is based on the bump type required of the prerelease
    // package to remain semver compliant relative to the new stable version.
    let candidate1 = stable_bump_instruction
        .map(|i| {
            match i.bump_type() {
                // Prerelease API is broken relative to stable. Need to major bump prerelease relative to
                // stable.
                BumpType::Major | BumpType::Minor => Some(
                    i.next_version
                        .bump(BumpType::Major, EndUserInitiated::No)
                        .with_prerelease(),
                ),
                // Stable API is not breaking relative to stable, so we can just bump the prerelease by
                // a patch to keep pace with the change in stable. But only if prerelease is not
                // already ahead of stable by minor or major or patch.
                BumpType::Patch => Some(
                    i.next_version
                        .bump(BumpType::Patch, EndUserInitiated::No)
                        .with_prerelease(),
                ),
            }
        })
        .flatten();

    // Second candidate for the bump type is based on the bump type of the prerelease parent
    let candidate2 = prerelease_parent_bump_instruction
        .map(|i| {
            match i.bump_type() {
                // Parent breaking change. Bump if not already bumped to be the stable version + major.
                BumpType::Major => Some(
                    cur_stable_version
                        .bump(BumpType::Major, EndUserInitiated::No)
                        .with_prerelease(),
                ),
                // Parent compatible change. Bump if not already bumped to be the stable major
                // minor or patch
                BumpType::Minor | BumpType::Patch => Some(
                    cur_stable_version
                        .bump(BumpType::Patch, EndUserInitiated::No)
                        .with_prerelease(),
                ),
            }
        })
        .flatten();

    let highest_candidate = match (candidate1.clone(), candidate2.clone()) {
        (Some(c1), Some(c2)) => Some(std::cmp::max(c1, c2)),
        (Some(c1), None) => Some(c1),
        (None, Some(c2)) => Some(c2),
        (None, None) => None,
    };

    highest_candidate
        .map(|v| {
            // Only return if current prerelease is not higher than our highest candidate here.
            if cur_prerelease_version >= v {
                None
            } else {
                Some(BumpInstruction {
                    package: prerelease_package.clone(),
                    next_version: v,
                })
            }
        })
        .flatten()
}

impl Display for BumpTree<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲\n"
        )?;
        write!(
            f,
            "🌲 Bump Tree (duplicates emitted, breaking bumps prioritised) 🌲\n"
        )?;
        write!(
            f,
            "🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲🌲\n"
        )?;
        for node in self.root_nodes.iter() {
            self.fmt_node(node, f, "".to_string(), true)?;
            write!(f, "\n\n")?;
        }
        let mut total_bumped = self.highest_stable.keys().collect::<HashSet<_>>();
        total_bumped.extend(self.highest_prerelease.keys().collect::<HashSet<_>>());
        write!(f, "Packages updated: {}", total_bumped.len())?;
        Ok(())
    }
}
