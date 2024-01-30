use cargo_metadata::DependencyKind;
use crates_io_api::AsyncClient;
use semver::Version;
use std::{collections::HashSet, fmt::Display, fs, path::PathBuf};
use toml_edit::{Document, Table};

/// A wrapper around the toml_edit Document with convenience methods
#[derive(Debug)]
pub struct Package {
    /// The doc
    doc: Document,
    /// Path
    path: PathBuf,
    /// Direct, non-development dependencies that are also workspace members
    pub direct_workspace_dependencies: HashSet<String>,
    /// Direct and indirect non-development dependencies that are also workspace members
    all_workspace_dependencies: Option<HashSet<String>>,
    /// Direct and indirect non-development dependents that are also workspace members
    all_workspace_dependents: Option<HashSet<String>>,
}

impl Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.version())
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

    pub fn set_version(self: &mut Self, version: &Version) {
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

    pub fn set_all_workspace_dependencies(
        self: &mut Self,
        all_workspace_dependencies: HashSet<String>,
    ) {
        self.all_workspace_dependencies = Some(all_workspace_dependencies);
    }

    pub fn all_workspace_dependencies(self: &Self) -> &HashSet<String> {
        self.all_workspace_dependencies
            .as_ref()
            .expect("all_workspace_dependencies not set")
    }

    pub fn set_all_workspace_dependents(
        self: &mut Self,
        all_workspace_dependents: HashSet<String>,
    ) {
        self.all_workspace_dependents = Some(all_workspace_dependents);
    }

    pub fn all_workspace_dependents(self: &Self) -> &HashSet<String> {
        self.all_workspace_dependents
            .as_ref()
            .expect("all_workspace_dependents not set")
    }

    pub fn new(
        cargo_metadata_package: &cargo_metadata::Package,
        workspace_members: &HashSet<String>,
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
            all_workspace_dependents: None,
            all_workspace_dependencies: None,
        })
    }
}
