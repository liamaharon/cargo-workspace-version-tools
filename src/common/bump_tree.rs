use super::version_extension::VersionExtension;
use super::workspace::Workspace;
use super::{package::Package, version_extension::BumpType};
use crate::common::logging::{BLUE, RED, RESET};
use core::fmt;
use semver::Version;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Eq, PartialEq)]
pub enum ReleaseChannel {
    Stable,
    Prerelease,
}

pub struct BumpTree<'a> {
    pub root_nodes: Vec<Rc<BumpNode>>,
    pub highest_stable: HashMap<String, Rc<BumpNode>>,
    pub highest_prerelease: HashMap<String, Rc<BumpNode>>,
    stable_workspace: &'a Workspace,
    prerelease_workspace: &'a Workspace,
}

#[derive(Debug, Clone)]
pub struct BumpNode {
    pub stable: Option<BumpInstruction>,
    pub prerelease: Option<BumpInstruction>,
    children: Vec<Rc<BumpNode>>,
}

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
                return Ok(None);
            }
        };
        let cur_stable_version = stable_package.borrow().version();

        match (release_channel, prerelease_workspace.packages.get(&name)) {
            // Stable is easy, just bump the version.
            (ReleaseChannel::Stable, _) => Ok(Some(BumpInstruction {
                package: stable_package.clone(),
                next_version: cur_stable_version.bump(semver_part),
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
                            return Ok(None);
                        }

                        // Need to bump to stable major+1
                        Ok(Some(BumpInstruction {
                            package: prerelease_package.clone(),
                            next_version: cur_stable_version
                                .bump(BumpType::Major)
                                .with_prerelease(),
                        }))
                    }
                    BumpType::Minor => {
                        // Ignore minor bump if already ahead on major or minor
                        if cur_prerelease_version.major > cur_stable_version.major
                            || cur_prerelease_version.minor > cur_stable_version.minor
                        {
                            return Ok(None);
                        }

                        // Need to bump to stable minor+1
                        Ok(Some(BumpInstruction {
                            package: prerelease_package.clone(),
                            next_version: cur_stable_version
                                .bump(BumpType::Minor)
                                .with_prerelease(),
                        }))
                    }
                    BumpType::Patch => {
                        // Ignore minor bump if already ahead on major or minor or patch
                        if cur_prerelease_version.major > cur_stable_version.major
                            || cur_prerelease_version.minor > cur_stable_version.minor
                            || cur_prerelease_version.patch > cur_stable_version.patch
                        {
                            return Ok(None);
                        }

                        // Need to bump to stable patch+1
                        Ok(Some(BumpInstruction {
                            package: prerelease_package.clone(),
                            next_version: cur_stable_version
                                .bump(BumpType::Patch)
                                .with_prerelease(),
                        }))
                    }
                }
            }
        }
    }
}

impl PartialEq for BumpNode {
    fn eq(&self, other: &Self) -> bool {
        self.stable == other.stable && self.prerelease == other.prerelease
    }
}

impl PartialEq for BumpInstruction {
    fn eq(&self, other: &Self) -> bool {
        self.package.borrow().name() == other.package.borrow().name()
            && self.package.borrow().branch == other.package.borrow().branch
            && self.bump_type() == other.bump_type()
    }
}

impl<'a> BumpTree<'a> {
    pub fn new(
        stable_workspace: &'a Workspace,
        prerelease_workspace: &'a Workspace,
        root_instructions: Vec<BumpInstruction>,
        release_channel: ReleaseChannel,
    ) -> Self {
        let mut tree = Self {
            root_nodes: vec![],
            highest_stable: HashMap::new(),
            highest_prerelease: HashMap::new(),
            stable_workspace,
            prerelease_workspace,
        };

        let root_nodes: Vec<_> = root_instructions
            .into_iter()
            .map(|i| match release_channel {
                ReleaseChannel::Prerelease => tree.new_node(None, Some(i)),
                ReleaseChannel::Stable => tree.new_node(
                    Some(i.clone()),
                    compute_prerelease_bump_instruction(
                        prerelease_workspace
                            .packages
                            .get(&i.package.borrow().name()),
                        stable_workspace.packages.get(&i.package.borrow().name()),
                        Some(&i),
                        None,
                    ),
                ),
            })
            .collect();

        tree.set_root_nodes(root_nodes);
        tree
    }

    fn set_root_nodes(&mut self, root_nodes: Vec<Rc<BumpNode>>) {
        self.root_nodes = root_nodes;
    }

    pub fn new_node(
        &mut self,
        stable_bump_instruction: Option<BumpInstruction>,
        prerelease_bump_instruction: Option<BumpInstruction>,
    ) -> Rc<BumpNode> {
        // Derive children
        let unique_children: HashSet<String> = stable_bump_instruction
            .iter()
            .chain(prerelease_bump_instruction.iter())
            .flat_map(|b| b.package.borrow().direct_workspace_dependents())
            .map(|dependent| dependent.borrow().name())
            .collect();

        let children_nodes = unique_children
            .into_iter()
            .map(|name| {
                self.derive_child_node(
                    stable_bump_instruction.as_ref(),
                    prerelease_bump_instruction.as_ref(),
                    self.stable_workspace.packages.get(&name),
                    self.prerelease_workspace.packages.get(&name),
                )
            })
            .collect();

        let bump_node = Rc::new(BumpNode {
            stable: stable_bump_instruction.clone(),
            prerelease: prerelease_bump_instruction.clone(),
            children: children_nodes,
        });

        // Update keeping track of the highest bumps we've seen for each package
        if let Some(stable_bump_instruction) = stable_bump_instruction {
            let name = stable_bump_instruction.package.borrow().name();
            self.highest_stable
                .entry(name.clone())
                .and_modify(|e| {
                    if e.stable.as_ref().map(|i| i.bump_type())
                        < Some(stable_bump_instruction.bump_type())
                    {
                        *e = bump_node.clone()
                    }
                })
                .or_insert(bump_node.clone());
        }

        if let Some(prerelease_bump_instruction) = prerelease_bump_instruction {
            let name = prerelease_bump_instruction.package.borrow().name();
            self.highest_prerelease
                .entry(name.clone())
                .and_modify(|e| {
                    if e.prerelease.as_ref().map(|i| i.bump_type())
                        < Some(prerelease_bump_instruction.bump_type())
                    {
                        *e = bump_node.clone()
                    }
                })
                .or_insert(bump_node.clone());
        }

        bump_node
    }

    pub fn derive_child_node(
        &mut self,
        stable_parent_bump_instruction: Option<&BumpInstruction>,
        prerelease_parent_bump_instruction: Option<&BumpInstruction>,
        stable_child_package: Option<&Rc<RefCell<Package>>>,
        prerelease_child_package: Option<&Rc<RefCell<Package>>>,
    ) -> Rc<BumpNode> {
        // Child stable bump type can be derived from the parent alone.
        //
        // If there's no parent bump, or no child package, the child bump type is just None.
        let stable_bump_instruction =
            if let (Some(stable_child_package), Some(stable_parent_instruction)) =
                (stable_child_package, stable_parent_bump_instruction)
            {
                let cur_version = stable_child_package.borrow().version();
                match stable_parent_instruction.bump_type() {
                    // Parent breaking change
                    BumpType::Major => Some(BumpInstruction {
                        package: stable_child_package.clone(),
                        next_version: cur_version.bump(BumpType::Major),
                    }),
                    // Parent compatible change
                    BumpType::Minor | BumpType::Patch => Some(BumpInstruction {
                        package: stable_child_package.clone(),
                        next_version: cur_version.bump(BumpType::Patch),
                    }),
                }
            } else {
                None
            };

        let prerelease_bump_instruction = compute_prerelease_bump_instruction(
            prerelease_child_package,
            stable_child_package,
            stable_bump_instruction.as_ref(),
            prerelease_parent_bump_instruction,
        );

        self.new_node(stable_bump_instruction, prerelease_bump_instruction)
    }

    fn fmt_node(
        &self,
        node: &Rc<BumpNode>,
        f: &mut Formatter,
        prefix: String,
        last: bool,
    ) -> fmt::Result {
        let is_root = prefix.is_empty();

        let connector = if is_root {
            ""
        } else if last {
            "└── "
        } else {
            "├── "
        };

        let stable_bump_details = if let Some(i) = &node.stable {
            let cur = i.package.borrow().version();
            let color = match i.bump_type() {
                BumpType::Major => RED,
                _ => BLUE,
            };
            format!(" stable({}{} -> {}{})", color, cur, i.next_version, RESET)
        } else {
            "".to_string()
        };

        let prerelease_bump_details = if let Some(i) = &node.prerelease {
            let cur = i.package.borrow().version();
            let color = match i.bump_type() {
                BumpType::Major => RED,
                _ => BLUE,
            };
            format!(
                " prerelease({}{} -> {}{})",
                color, cur, i.next_version, RESET
            )
        } else {
            "".to_string()
        };
        write!(
            f,
            "{}{}{}{}{}",
            prefix,
            connector,
            node.package_name(),
            stable_bump_details,
            prerelease_bump_details,
        )?;

        let new_prefix = if last {
            prefix + "    "
        } else {
            prefix + "│   "
        };
        let significant_children = node
            .children
            .iter()
            .filter(|c| {
                let highest_stable_child = self.highest_stable.get(&c.package_name());
                let highest_prerelease_child = self.highest_prerelease.get(&c.package_name());
                highest_stable_child.is_some_and(|highest| Rc::ptr_eq(c, highest))
                    || highest_prerelease_child.is_some_and(|highest| Rc::ptr_eq(c, highest))
            })
            .collect::<Vec<_>>();
        for (i, dependent) in significant_children.iter().enumerate() {
            let is_last = i == significant_children.len() - 1;
            write!(f, "\n")?;
            self.fmt_node(dependent, f, new_prefix.clone(), is_last)?;
        }

        Ok(())
    }
}

/// Prerelease bump type is influenced by the parent and the next stable bump type.
/// It also requires a stable package to exist for this child, otherwise the prerelease
/// isn't being bumped in relation to anything.
fn compute_prerelease_bump_instruction(
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
                BumpType::Major | BumpType::Minor => {
                    Some(i.next_version.bump(BumpType::Major).with_prerelease())
                }
                // Stable API is not breaking relative to stable, so we can just bump the prerelease by
                // a patch to keep pace with the change in stable. But only if prerelease is not
                // already ahead of stable by minor or major.
                BumpType::Patch => {
                    if cur_prerelease_version.major == cur_stable_version.major
                        && cur_prerelease_version.minor == cur_stable_version.minor
                    {
                        Some(i.next_version.bump(BumpType::Patch).with_prerelease())
                    } else {
                        None
                    }
                }
            }
        })
        .flatten();

    // Second candidate for the bump type is based on the bump type of the prerelease parent
    let candidate2 = prerelease_parent_bump_instruction.map(|i| {
        match i.bump_type() {
            // Parent breaking change. Bump if not already bumped to be the stable version + major.
            BumpType::Major => cur_prerelease_version
                .bump(BumpType::Major)
                .with_prerelease(),
            // Parent compatible change
            BumpType::Minor | BumpType::Patch => cur_prerelease_version
                .bump(BumpType::Patch)
                .with_prerelease(),
        }
    });

    let highest_candidate = match (candidate1.clone(), candidate2.clone()) {
        (Some(c1), Some(c2)) => Some(std::cmp::max(c1, c2)),
        (Some(c1), None) => Some(c1),
        (None, Some(c2)) => Some(c2),
        (None, None) => None,
    };

    highest_candidate.map(|v| BumpInstruction {
        package: prerelease_package.clone(),
        next_version: v,
    })
}

impl BumpNode {
    pub fn package_name(&self) -> String {
        if let Some(i) = &self.stable {
            i.package.borrow().name()
        } else if let Some(i) = &self.prerelease {
            i.package.borrow().name()
        } else {
            panic!("One of stable or prerelease must be set")
        }
    }
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
