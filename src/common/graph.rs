use super::colors::{BLUE, GREY, RED};
use super::{package::Package, workspace::Workspace};
use crate::common::colors::RESET;
use crate::common::version_extension::{BumpType, VersionExtension};
use core::fmt;
use semver::Version;
use std::collections::{HashMap, HashSet};
use std::{
    cell::RefCell,
    fmt::{Display, Formatter},
    rc::Rc,
};

pub struct BumpNode {
    package: Rc<RefCell<Package>>,
    next_version: Version,
    bump_type: BumpType,
    /// Current and next prerelease version of the package.
    prerelease_bump_details: Option<(Version, Version)>,
    dependents: Vec<BumpNode>,
}

impl BumpNode {
    fn fmt_with_indent(&self, f: &mut Formatter, prefix: String, last: bool) -> fmt::Result {
        let is_root = prefix.is_empty();
        let color = match self.bump_type {
            BumpType::Compatible => BLUE,
            BumpType::Breaking => RED,
        };

        let connector = if is_root {
            ""
        } else if last {
            "└── "
        } else {
            "├── "
        };

        let prerelease_bump_details = if let Some((cur, next)) = &self.prerelease_bump_details {
            format!(" prerelease({} -> {})", cur, next)
        } else {
            "".to_string()
        };
        write!(
            f,
            "{}{}{}{} stable({} -> {}){}{}",
            prefix,
            connector,
            self.package.borrow().name(),
            color,
            self.package.borrow().version(),
            self.next_version,
            prerelease_bump_details,
            RESET
        )?;

        // Continue with the logic for dependents as before.
        let new_prefix = if last {
            prefix + "    "
        } else {
            prefix + "│   "
        };
        for (i, dependent) in self.dependents.iter().enumerate() {
            let is_last = i == self.dependents.len() - 1;
            write!(f, "\n")?;
            dependent.fmt_with_indent(f, new_prefix.clone(), is_last)?;
        }

        Ok(())
    }
}

impl Display for BumpNode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.fmt_with_indent(f, "".to_string(), true)
    }
}

pub struct BumpSummary {
    pub compatible_bumps: usize,
    pub breaking_bumps: usize,
}

impl Display for BumpSummary {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "[Summary] {}Compatible: {} {}Breaking: {}{} Total: {}",
            BLUE,
            self.compatible_bumps,
            RED,
            self.breaking_bumps,
            RESET,
            self.compatible_bumps + self.breaking_bumps
        )
    }
}

pub struct BumpDetails {
    pub next_version: Version,
    pub next_prerelease_version: Option<Version>,
    pub bump_type: BumpType,
}

// Define `BumpTree` with a lifetime parameter `'a`
pub struct BumpTree<'a> {
    stable_workspace: &'a Workspace,
    prerelease_workspace: &'a Option<Workspace>,
    pub bumped: HashMap<String, BumpDetails>,
}

impl<'a> BumpTree<'a> {
    pub fn new(
        stable_workspace: &'a Workspace,
        prerelease_workspace: &'a Option<Workspace>,
    ) -> Self {
        Self {
            stable_workspace,
            prerelease_workspace,
            bumped: HashMap::new(),
        }
    }

    pub fn build(
        self: &mut Self,
        package: Rc<RefCell<Package>>,
        next_version: Version,
    ) -> Option<BumpNode> {
        let bump_type = next_version.bump_type(&package.borrow().version());
        let existing_bump = self.bumped.get(&package.borrow().name().to_owned());
        let do_bump = match existing_bump {
            Some(b) => b.bump_type == BumpType::Compatible && bump_type == BumpType::Breaking,
            None => true,
        };

        if do_bump {
            let cur_version = package.borrow().version();

            // Handle bumping prerelease version to keep them up to date with stable.
            let pre_release_package = self
                .prerelease_workspace
                .as_ref()
                .and_then(|p| p.packages.get(&package.borrow().name().to_owned()));

            let prerelease_bump_details = if let Some(pre_release_package) = pre_release_package {
                // Bump prereleases the same amount as the stable version so the diff remains the
                // same.
                let mut next_pre_release_version = pre_release_package.borrow().version().clone();
                next_pre_release_version.major += next_version.major - cur_version.major;
                next_pre_release_version.minor += next_version.minor - cur_version.minor;
                next_pre_release_version.patch += next_version.patch - cur_version.patch;
                Some((
                    pre_release_package.borrow().version().clone(),
                    next_pre_release_version,
                ))
            } else {
                None
            };

            let bump = BumpDetails {
                next_version: next_version.clone(),
                next_prerelease_version: prerelease_bump_details.clone().map(|d| d.1),
                bump_type,
            };
            self.bumped.insert(package.borrow().name().to_owned(), bump);

            let mut dependent_nodes = vec![];
            for dependent_name in package.borrow().direct_workspace_dependents() {
                let dependent_package = self
                    .stable_workspace
                    .packages
                    .get(dependent_name)
                    .expect("Package must exist");

                let dependent_next_version = match bump_type {
                    BumpType::Breaking => dependent_package.borrow().version().bump_breaking(),
                    BumpType::Compatible => dependent_package.borrow().version().bump_smallest().0,
                };

                let dependent_node = self.build(dependent_package.clone(), dependent_next_version);

                if let Some(dependent_node) = dependent_node {
                    dependent_nodes.push(dependent_node);
                }
            }

            return Some(BumpNode {
                package: package.clone(),
                bump_type,
                next_version,
                prerelease_bump_details,
                dependents: dependent_nodes,
            });
        }

        None
    }

    pub fn summary(&self) -> BumpSummary {
        let (compatible_num, breaking_num) =
            self.bumped
                .values()
                .fold((0, 0), |acc, b| match b.bump_type {
                    BumpType::Compatible => (acc.0 + 1, acc.1),
                    BumpType::Breaking => (acc.0, acc.1 + 1),
                });
        BumpSummary {
            compatible_bumps: compatible_num,
            breaking_bumps: breaking_num,
        }
    }
}

/// Finds all direct dependents of a given package.
pub fn find_direct_dependents(
    package: &str,
    workspace_deps: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut dependents = HashSet::new();
    for (name, deps) in workspace_deps {
        if deps.contains(package) {
            dependents.insert(name.clone());
        }
    }
    dependents
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// Simple dependency graph
    /// package_a depends on package_b and package_c
    /// package_b depends on package_c
    /// package_c has no dependencies
    fn create_mock_workspace_deps() -> HashMap<String, HashSet<String>> {
        let mut workspace_deps = HashMap::new();

        workspace_deps.insert(
            "package_a".to_string(),
            HashSet::from(["package_b".to_string(), "package_c".to_string()]),
        );
        workspace_deps.insert(
            "package_b".to_string(),
            HashSet::from(["package_c".to_string()]),
        );
        workspace_deps.insert("package_c".to_string(), HashSet::new());

        workspace_deps
    }

    #[test]
    fn test_find_direct_dependents() {
        let workspace_deps = create_mock_workspace_deps();
        let direct_dependents_c = find_direct_dependents("package_c", &workspace_deps);
        assert!(direct_dependents_c.contains("package_a"));
        assert!(direct_dependents_c.contains("package_b"));
        assert_eq!(direct_dependents_c.len(), 2);
        let direct_dependents_b = find_direct_dependents("package_b", &workspace_deps);
        assert!(direct_dependents_b.contains("package_a"));
        assert_eq!(direct_dependents_b.len(), 1);
        let direct_dependents_a = find_direct_dependents("package_a", &workspace_deps);
        assert!(direct_dependents_a.is_empty());
    }
}
