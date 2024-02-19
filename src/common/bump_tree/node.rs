use super::instruction::BumpInstruction;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct BumpNode {
    pub stable: Option<BumpInstruction>,
    pub prerelease: Option<BumpInstruction>,
    pub children: Vec<Rc<BumpNode>>,
}

impl PartialEq for BumpNode {
    fn eq(&self, other: &Self) -> bool {
        self.stable == other.stable && self.prerelease == other.prerelease
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
