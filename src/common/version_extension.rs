use semver::{Prerelease, Version};
use std::{cmp::Ordering, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BumpType {
    Major,
    Minor,
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndUserInitiated {
    Yes,
    No,
}

impl BumpType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "major" => Ok(BumpType::Major),
            "minor" => Ok(BumpType::Minor),
            "patch" => Ok(BumpType::Patch),
            _ => Err(format!("Invalid bump type: {}", s)),
        }
    }
}

impl PartialOrd for BumpType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (BumpType::Major, BumpType::Major) => Some(Ordering::Equal),
            (BumpType::Major, _) => Some(Ordering::Greater),

            (BumpType::Minor, BumpType::Major) => Some(Ordering::Less),
            (BumpType::Minor, BumpType::Minor) => Some(Ordering::Equal),
            (BumpType::Minor, BumpType::Patch) => Some(Ordering::Greater),

            (BumpType::Patch, BumpType::Patch) => Some(Ordering::Equal),
            (BumpType::Patch, _) => Some(Ordering::Less),
        }
    }
}

impl Ord for BumpType {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

pub trait VersionExtension {
    fn bump(self: &Self, bump_type: BumpType, end_user_initiated: EndUserInitiated) -> Version;
    fn with_prerelease(self: &Self) -> Version;
}

impl VersionExtension for Version {
    fn bump(self: &Self, bump_type: BumpType, end_user_initiated: EndUserInitiated) -> Version {
        let mut next_version = self.clone();
        match bump_type {
            BumpType::Major => match end_user_initiated {
                // If the end-user initiated the bump, we assume they are intentional in requesting
                // to bump to at least v1.0.0
                EndUserInitiated::Yes => {
                    next_version.major += 1;
                    next_version.minor = 0;
                    next_version.patch = 0;
                }
                // Otherwise, we can make a 'major' (incompatible) bump by just bumping minor
                EndUserInitiated::No => {
                    if self.major > 0 {
                        next_version.major += 1;
                        next_version.minor = 0;
                    } else {
                        next_version.minor += 1;
                    }
                    next_version.patch = 0;
                }
            },
            BumpType::Minor => {
                if end_user_initiated == EndUserInitiated::Yes && self.major == 0 {
                    log::info!("ℹ Note: Bumping minor on a package with a major version of 0 is a breaking change. You may consider making a major or patch bump instead.");
                }
                next_version.minor += 1;
                next_version.patch = 0;
            }
            BumpType::Patch => {
                next_version.patch += 1;
            }
        };
        next_version
    }

    fn with_prerelease(self: &Self) -> Version {
        let mut next_version = self.clone();
        next_version.pre = Prerelease::from_str("alpha").expect("valid");
        next_version
    }
}

#[test]
fn bump_type_ordering() {
    assert!(BumpType::Major > BumpType::Minor);
    assert!(BumpType::Major > BumpType::Patch);
    assert!(BumpType::Minor > BumpType::Patch);
    assert!(BumpType::Minor < BumpType::Major);
    assert!(BumpType::Patch < BumpType::Major);
    assert!(BumpType::Patch < BumpType::Minor);
    assert!(BumpType::Major == BumpType::Major);
    assert!(BumpType::Minor == BumpType::Minor);
    assert!(BumpType::Patch == BumpType::Patch);
    assert!(std::cmp::max(BumpType::Major, BumpType::Minor) == BumpType::Major);
}
