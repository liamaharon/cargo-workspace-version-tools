use core::fmt;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::{cell::RefCell, rc::Rc};

use crate::common::logging::{BLUE, RED, RESET};

use super::version_extension::VersionExtension;
use super::workspace::Workspace;
use super::{package::Package, version_extension::BumpType};

#[derive(Debug, Eq, PartialEq)]
pub enum BranchType {
    Stable,
    Prerelease,
}

pub struct BumpTree<'a> {
    root_nodes: Vec<Rc<BumpNode>>,
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
    pub bump_type: BumpType,
}

impl BumpInstruction {
    pub fn from_str(
        stable_workspace: &Workspace,
        prerelease_workspace: &Workspace,
        s: &str,
        branch_type: &BranchType,
    ) -> Result<BumpInstruction, String> {
        let parts: Vec<&str> = s.splitn(2, ' ').collect();
        let name = parts[0].to_string();
        let semver_part = parts
            .get(1)
            .map(|b| BumpType::from_str(b))
            .unwrap_or_else(|| Err(format!("Invalid Bump Instruction: '{}'", s).to_string()))?;

        let package = match branch_type {
            BranchType::Stable => stable_workspace.packages.get(&name).ok_or(format!(
                "Package {} not found on branch {}",
                name, stable_workspace.branch_name
            )),
            BranchType::Prerelease => prerelease_workspace.packages.get(&name).ok_or(format!(
                "Package {} not found on branch {}",
                name, prerelease_workspace.branch_name
            )),
        }?;

        Ok(BumpInstruction {
            package: package.clone(),
            bump_type: semver_part,
        })
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
            && self.bump_type == other.bump_type
    }
}

impl<'a> BumpTree<'a> {
    pub fn new(
        stable_workspace: &'a Workspace,
        prerelease_workspace: &'a Workspace,
        root_instructions: Vec<BumpInstruction>,
        branch_type: BranchType,
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
            .map(|i| match branch_type {
                // Prerelease is easy, there's never any bump instruction for stable.
                BranchType::Prerelease => tree.new_node(None, Some(i)),
                // Stable bumps may also involve prerelease bumps
                BranchType::Stable => {
                    let prerelease_bump_instruction = compute_prerelease_bump_instruction(
                        prerelease_workspace
                            .packages
                            .get(&i.package.borrow().name()),
                        Some(&i.package),
                        Some(&i),
                        None,
                    );
                    tree.new_node(Some(i), prerelease_bump_instruction)
                }
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
                    if e.stable.as_ref().map(|i| i.bump_type)
                        < Some(stable_bump_instruction.bump_type)
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
                    if e.prerelease.as_ref().map(|i| i.bump_type)
                        < Some(prerelease_bump_instruction.bump_type)
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
                match stable_parent_instruction.bump_type {
                    // Parent breaking change
                    BumpType::Major => Some(BumpInstruction {
                        package: stable_child_package.clone(),
                        bump_type: BumpType::Major,
                    }),
                    // Parent compatible change
                    BumpType::Minor | BumpType::Patch => Some(BumpInstruction {
                        package: stable_child_package.clone(),
                        bump_type: BumpType::Patch,
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
            let next = cur.bump(&i.bump_type);
            let color = match i.bump_type {
                BumpType::Major => RED,
                _ => BLUE,
            };
            format!(" stable({}{} -> {}{})", color, cur, next, RESET)
        } else {
            "".to_string()
        };

        let prerelease_bump_details = if let Some(i) = &node.prerelease {
            let cur = i.package.borrow().version();
            let next = cur.bump(&i.bump_type);
            let color = match i.bump_type {
                BumpType::Major => RED,
                _ => BLUE,
            };
            format!(" prerelease({}{} -> {}{})", color, cur, next, RESET)
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
                return highest_stable_child.is_some_and(|highest| Rc::ptr_eq(c, highest))
                    || highest_prerelease_child.is_some_and(|highest| Rc::ptr_eq(c, highest));
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
    prerelease_child_package: Option<&Rc<RefCell<Package>>>,
    stable_child_package: Option<&Rc<RefCell<Package>>>,
    stable_bump_instruction: Option<&BumpInstruction>,
    prerelease_parent_bump_instruction: Option<&BumpInstruction>,
) -> Option<BumpInstruction> {
    if let (Some(prerelease_child_package), Some(stable_child_package)) =
        (prerelease_child_package, stable_child_package)
    {
        // First candidate for the bump type is based on the bump type of stable
        let candidate1 = match stable_bump_instruction.map(|i| i.bump_type) {
            // Prerelease API got broken relative to the stable change
            Some(BumpType::Major) | Some(BumpType::Minor) => Some(BumpType::Major),
            // Prerelease API compatible with the stable change
            Some(BumpType::Patch) => Some(BumpType::Patch),
            None => None,
        };

        // Second candidate for the bump type is based on the bump type of the prerelease parent
        let candidate2 =
            if let Some(prerelease_parent_instruction) = prerelease_parent_bump_instruction {
                match prerelease_parent_instruction.bump_type {
                    // Parent breaking change
                    BumpType::Major => Some(BumpType::Major),
                    // Parent compatible change
                    BumpType::Minor | BumpType::Patch => Some(BumpType::Patch),
                }
            } else {
                None
            };

        let highest_candidate = match (candidate1, candidate2) {
            (Some(c1), Some(c2)) => Some(std::cmp::max(c1, c2)),
            (Some(c1), None) => Some(c1),
            (None, Some(c2)) => Some(c2),
            (None, None) => None,
        };

        // If we have a bump type to consider, only apply it if it hasn't yet
        let stable_version = stable_child_package.borrow().version();
        let prerelease_version = prerelease_child_package.borrow().version();
        let final_bump_type = match highest_candidate {
            Some(BumpType::Major) if prerelease_version.major == stable_version.major => {
                Some(BumpType::Major)
            }
            Some(BumpType::Minor) if prerelease_version.minor == stable_version.minor => {
                Some(BumpType::Minor)
            }
            Some(BumpType::Patch) if prerelease_version.patch == stable_version.patch => {
                Some(BumpType::Patch)
            }
            _ => None,
        };

        final_bump_type.map(|bump_type| BumpInstruction {
            package: prerelease_child_package.clone(),
            bump_type,
        })
    } else {
        None
    }
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
