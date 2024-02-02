use super::{package::Package, workspace::Workspace};
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
    already_checked: bool,
    dependents: Vec<BumpNode>,
}

const RED: &str = "\x1b[31m";
const GREY: &str = "\x1b[90m";
const BLUE: &str = "\x1b[34m";
const RESET: &str = "\x1b[0m"; // Resets the color

impl BumpNode {
    fn fmt_with_indent(&self, f: &mut Formatter, prefix: String, last: bool) -> fmt::Result {
        let is_root = prefix.is_empty();
        let color = if self.already_checked {
            GREY
        } else if self.bump_type == BumpType::Compatible {
            BLUE
        } else {
            RED
        };

        if is_root {
            // For the root node, omit the connector.
            write!(
                f,
                "{}{}{} ({} -> {}){}",
                prefix,
                color,
                self.package.borrow().name(),
                self.package.borrow().version(),
                self.next_version,
                RESET
            )?;
        } else {
            // For non-root nodes, include the connector.
            let connector = if last { "└── " } else { "├── " };
            write!(
                f,
                "{}{}{}{} ({} -> {}){}",
                prefix,
                connector,
                if self.already_checked { GREY } else { color },
                self.package.borrow().name(),
                self.package.borrow().version(),
                self.next_version,
                RESET
            )?;
        }

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

// Define `BumpTree` with a lifetime parameter `'a`
pub struct BumpTree<'a> {
    workspace: &'a Workspace,
    bumped: HashSet<(String, BumpType)>,
}

impl<'a> BumpTree<'a> {
    pub fn new(workspace: &'a Workspace) -> Self {
        Self {
            workspace,
            bumped: HashSet::new(),
        }
    }

    pub fn build(
        self: &mut Self,
        package: Rc<RefCell<Package>>,
        next_version: Version,
    ) -> BumpNode {
        let bump_type = next_version.bump_type(&package.borrow().version());
        let already_checked = self
            .bumped
            .contains(&(package.borrow().name().to_owned(), bump_type));

        self.bumped
            .insert((package.borrow().name().to_owned(), bump_type));

        let mut dependent_nodes = vec![];
        if !already_checked {
            for dependent_name in package.borrow().direct_workspace_dependents() {
                let dependent_package = self
                    .workspace
                    .packages
                    .get(dependent_name)
                    .expect("Package must exist");

                let dependent_next_version = match bump_type {
                    BumpType::Breaking => dependent_package.borrow().version().bump_breaking(),
                    BumpType::Compatible => dependent_package.borrow().version().bump_smallest().0,
                };

                let dependent_node = self.build(dependent_package.clone(), dependent_next_version);

                if !dependent_node.already_checked {
                    dependent_nodes.push(dependent_node);
                }
            }
        }

        BumpNode {
            package: package.clone(),
            bump_type,
            next_version,
            dependents: dependent_nodes,
            already_checked,
        }
    }
}

/// Finds all dependencies (both direct and indirect) of a given package.
fn _find_dependencies(
    package: &str,
    workspace_deps: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut dependencies = HashSet::new();
    let mut stack = vec![package.to_string()];
    let mut visited = HashSet::new();

    while let Some(current_package) = stack.pop() {
        if visited.insert(current_package.clone()) {
            let deps = workspace_deps
                .get(&current_package)
                .expect("Package must be in workspace");

            for dep in deps {
                dependencies.insert(dep.clone());
                stack.push(dep.clone());
            }
        }
    }

    dependencies
}

/// Finds all dependents (both direct and indirect) of a given package.
fn _find_dependents(
    package: &str,
    workspace_deps: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut dependents = HashSet::new();
    let mut stack = vec![package.to_string()];
    let mut visited = HashSet::new();

    while let Some(current_package) = stack.pop() {
        if visited.insert(current_package.clone()) {
            for (name, deps) in workspace_deps {
                if deps.contains(&current_package) {
                    dependents.insert(name.clone());
                    stack.push(name.clone());
                }
            }
        }
    }

    dependents
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

    #[test]
    fn test_find_dependencies() {
        let workspace_deps = create_mock_workspace_deps();

        let dependencies_a = _find_dependencies("package_a", &workspace_deps);
        assert!(dependencies_a.contains("package_b"));
        assert!(dependencies_a.contains("package_c"));
        assert_eq!(dependencies_a.len(), 2);

        let dependencies_b = _find_dependencies("package_b", &workspace_deps);
        assert!(dependencies_b.contains("package_c"));
        assert_eq!(dependencies_b.len(), 1);

        let dependencies_c = _find_dependencies("package_c", &workspace_deps);
        assert!(dependencies_c.is_empty());
    }

    #[test]
    fn test_find_dependents() {
        let workspace_deps = create_mock_workspace_deps();

        let dependents_c = _find_dependents("package_c", &workspace_deps);
        assert!(dependents_c.contains("package_a"));
        assert!(dependents_c.contains("package_b"));
        assert_eq!(dependents_c.len(), 2);

        let dependents_b = _find_dependents("package_b", &workspace_deps);
        assert!(dependents_b.contains("package_a"));
        assert_eq!(dependents_b.len(), 1);

        let dependents_a = _find_dependents("package_a", &workspace_deps);
        assert!(dependents_a.is_empty());
    }
}
