use cargo_metadata::DependencyKind;
use crates_io_api::AsyncClient;
use semver::Version;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Display,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    rc::Rc,
};
use toml_edit::{Document, Table};

/// A wrapper around the toml_edit Document with convenience methods
#[derive(Debug)]
pub struct Package {
    /// The doc
    doc: Document,
    /// Path
    path: PathBuf,
    /// Direct, non-development dependencies that are also workspace members
    direct_workspace_dependencies: HashSet<String>,
    /// Direct, non-development dependents that are also workspace members
    direct_workspace_dependents: Option<HashMap<String, Rc<RefCell<Package>>>>,
    /// Branch name
    pub branch: String,
}

impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.branch == other.branch
    }
}

impl Eq for Package {}

impl Hash for Package {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.branch.hash(state);
    }
}

impl Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Package {
    fn package(self: &Self) -> &Table {
        self.doc
            .get("package")
            .and_then(|p| p.as_table())
            .expect(format!("Package {:?} is missing [package] table", self.path).as_str())
    }

    fn package_mut(self: &mut Self) -> &mut Table {
        self.doc
            .get_mut("package")
            .and_then(|p| p.as_table_mut())
            .expect(format!("Package {:?} is missing [package] table", self.path).as_str())
    }

    pub fn name(self: &Self) -> String {
        self.package()
            .get("name")
            .and_then(|n| n.as_str())
            .expect(format!("Package {:?} has invalid name", self.path).as_str())
            .to_owned()
    }

    pub fn version(self: &Self) -> Version {
        let version_str = self
            .package()
            .get("version")
            .and_then(|v| v.as_str())
            .expect(format!("Package {:?} has invalid version", self.path).as_str());

        Version::parse(version_str)
            .expect(format!("Failed to create Version from {:?} version", self.path).as_str())
    }

    pub fn direct_workspace_dependents(&self) -> impl Iterator<Item = Rc<RefCell<Package>>> {
        let a = self
            .direct_workspace_dependents
            .as_ref()
            .expect("Direct dependents not initialized")
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .into_iter();

        a
    }

    pub fn direct_workspace_dependencies(&self) -> &HashSet<String> {
        &self.direct_workspace_dependencies
    }

    pub fn set_direct_dependents(
        self: &mut Self,
        direct_dependents: HashMap<String, Rc<RefCell<Package>>>,
    ) {
        self.direct_workspace_dependents = Some(direct_dependents);
    }

    pub fn set_version(self: &mut Self, version: &Version) {
        log::debug!("Bumping {} to {}", self.name(), version);

        self.package_mut()["version"] = toml_edit::value(version.to_string());
        fs::write(self.path.clone(), self.doc.to_string())
            .expect(format!("Failed to write to {:?}", self.path).as_str())
    }

    pub async fn crates_io_version(self: &Self, client: &AsyncClient) -> Result<Version, String> {
        let crates_io_version_str = client
            .get_crate(self.name().as_str())
            .await
            .map_err(|e| format!("Failed to get crate from crates.io: {}", e))?
            .crate_data
            .max_version;

        Ok(Version::parse(crates_io_version_str.as_str())
            .expect(format!("crates.io returned bad version for crate {}", self.name()).as_str()))
    }

    pub fn publish(self: &Self) -> bool {
        if let Some(publish) = self.package().get("publish").and_then(|p| p.as_bool()) {
            if !publish {
                return false;
            }
        }
        return true;
    }

    pub fn new(
        cargo_metadata_package: &cargo_metadata::Package,
        workspace_members: &HashSet<String>,
        branch: &str,
    ) -> Result<Self, String> {
        let path = cargo_metadata_package.manifest_path.clone();
        let content = fs::read_to_string(&path).map_err(|e| {
            format!(
                "Failed to read Cargo.toml for package at path {:?}: {}",
                path, e
            )
        })?;
        let doc = content
            .parse::<Document>()
            .map_err(|e| format!("Package Cargo.toml at path {:?} is invalid: {}", path, e))?;

        Ok(Self {
            doc,
            branch: branch.to_owned(),
            direct_workspace_dependents: None,
            direct_workspace_dependencies: cargo_metadata_package
                .dependencies
                .iter()
                .filter(|d| {
                    workspace_members.contains(d.name.as_str())
                        && d.kind != DependencyKind::Development
                })
                .map(|d| d.name.clone())
                .collect(),
            path: path.into(),
        })
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
