use core::fmt;
use std::fmt::{Display, Formatter};

use semver::{Version, VersionReq};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BumpType {
    Breaking,
    Compatible,
}

impl Display for BumpType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BumpType::Breaking => write!(f, "Breaking"),
            BumpType::Compatible => write!(f, "Compatible"),
        }
    }
}

pub trait VersionExtension {
    fn bump_type(self: &Self, version: &Version) -> BumpType;

    fn bump_smallest(self: &Self) -> (Version, BumpType);

    fn bump_breaking(self: &Self) -> Version;

    fn bump_prerelease(self: &Self) -> Version;
}

impl VersionExtension for Version {
    /// Checks if the given version is compatible with the current version.
    fn bump_type(self: &Self, version: &Version) -> BumpType {
        // ~version actually will treat prerelease bumps compatible, see
        // https://internals.rust-lang.org/t/changing-cargo-semver-compatibility-for-pre-releases/14820
        if !self.pre.is_empty() && version != self {
            return BumpType::Breaking;
        }

        let compatible_requirement = VersionReq::parse(format!("^{}", version).as_str())
            .expect("Version was just serialised, qed");
        if compatible_requirement.matches(&self) {
            BumpType::Compatible
        } else {
            BumpType::Breaking
        }
    }

    fn bump_prerelease(self: &Self) -> Version {
        let mut next_version = self.clone();

        // Deal with version without prerelease yet
        if self.pre.is_empty() {
            next_version.pre = "alpha.1".parse().unwrap();
            return next_version;
        }

        // Deal with existing prerelease version
        let pre_string = self.pre.to_string();
        let pre_parts = pre_string.split(".").collect::<Vec<_>>();

        if pre_parts.len() != 2 || pre_parts[1].parse::<u64>().is_err() {
            log::warn!("Prerelease version {} is not in the format of somelabel.n. Appending .1 to the prerelease part and moving on.", &self);
            next_version.pre = format!("{}.1", self.pre).parse().unwrap();
            return next_version;
        }

        next_version.pre = format!(
            "{}.{}",
            pre_parts[0],
            pre_parts
                .get(1)
                .unwrap_or(&"0")
                .parse::<u64>()
                .expect("must be valid")
                + 1
        )
        .parse()
        .expect("Just serialised");
        next_version
    }

    fn bump_breaking(self: &Self) -> Version {
        let mut next_version = self.clone();
        if self.pre.is_empty() {
            if next_version.major > 0 {
                next_version.major += 1;
                next_version.minor = 0;
                next_version.patch = 0;
            } else if next_version.minor > 0 {
                next_version.minor += 1;
                next_version.patch = 0;
            } else {
                next_version.patch += 1;
            }
            return next_version;
        }

        self.bump_prerelease()
    }

    /// Makes a patch bump if possible, otherwise a breaking bump.
    ///
    /// Returns a tuple of the bumped version and whether the bump would be compatible or
    /// breaking.
    fn bump_smallest(self: &Self) -> (Version, BumpType) {
        let mut bumped = self.clone();

        // Deal with non-prerelease 0.0.X version
        if bumped.pre.is_empty() && bumped.minor == 0 && bumped.major == 0 {
            bumped.patch += 1;
            return (bumped, BumpType::Breaking);
        };

        // Deal with prerelease version
        if !bumped.pre.is_empty() {
            let pre_string = bumped.pre.to_string();
            let pre_parts = pre_string.split(".").collect::<Vec<_>>();

            // Check parts
            if pre_parts.len() != 2 || pre_parts[1].parse::<u64>().is_err() {
                log::warn!("Prerelease version  is not in the format of somelabel.n. Appending .1 to the prerelease part and moving on.");
                bumped.pre = format!("{}.1", bumped.pre).parse().unwrap();
            }

            // Bump the number and return
            bumped.pre = format!(
                "{}.{}",
                pre_parts[0],
                pre_parts
                    .get(1)
                    .unwrap_or(&"0")
                    .parse::<u64>()
                    .expect("Just checked the number")
                    + 1
            )
            .parse()
            .expect("Just serialised");

            return (bumped, BumpType::Breaking);
        };

        // Simply bump the patch version
        bumped.patch += 1;
        (bumped, BumpType::Compatible)
    }
}
