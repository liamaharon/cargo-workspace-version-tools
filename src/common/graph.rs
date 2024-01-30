use std::collections::{HashMap, HashSet};

/// Finds all dependencies (both direct and indirect) of a given package.
pub fn find_dependencies(
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
pub fn find_dependents(
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
    fn test_find_dependencies() {
        let workspace_deps = create_mock_workspace_deps();

        let dependencies_a = find_dependencies("package_a", &workspace_deps);
        assert!(dependencies_a.contains("package_b"));
        assert!(dependencies_a.contains("package_c"));
        assert_eq!(dependencies_a.len(), 2);

        let dependencies_b = find_dependencies("package_b", &workspace_deps);
        assert!(dependencies_b.contains("package_c"));
        assert_eq!(dependencies_b.len(), 1);

        let dependencies_c = find_dependencies("package_c", &workspace_deps);
        assert!(dependencies_c.is_empty());
    }

    #[test]
    fn test_find_dependents() {
        let workspace_deps = create_mock_workspace_deps();

        let dependents_c = find_dependents("package_c", &workspace_deps);
        assert!(dependents_c.contains("package_a"));
        assert!(dependents_c.contains("package_b"));
        assert_eq!(dependents_c.len(), 2);

        let dependents_b = find_dependents("package_b", &workspace_deps);
        assert!(dependents_b.contains("package_a"));
        assert_eq!(dependents_b.len(), 1);

        let dependents_a = find_dependents("package_a", &workspace_deps);
        assert!(dependents_a.is_empty());
    }
}
