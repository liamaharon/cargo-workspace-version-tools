use super::instruction::{compute_prerelease_bump_instruction, BumpInstruction};
use super::node::BumpNode;
use crate::common::logging::{BLUE, RED, RESET};
use crate::common::package::Package;
use crate::common::version_extension::VersionExtension;
use crate::common::version_extension::{BumpType, EndUserInitiated};
use crate::common::workspace::Workspace;
use core::fmt;
use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;
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
                        next_version: cur_version.bump(BumpType::Major, EndUserInitiated::No),
                    }),
                    // Parent compatible change
                    BumpType::Minor | BumpType::Patch => Some(BumpInstruction {
                        package: stable_child_package.clone(),
                        next_version: cur_version.bump(BumpType::Patch, EndUserInitiated::No),
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

    pub fn fmt_node(
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
