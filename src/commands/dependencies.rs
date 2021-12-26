use crate::commands::available_packages;
use crate::commands::packages::Package;
use ansi_term::Color;
use regex::Regex;
use std::collections::{BTreeSet, VecDeque};

// Returns the packages in installation order. Dependencies should be installed before dependents.
pub fn list(
    packages: Vec<&Package>,
    installed_packages: &BTreeSet<Package>,
    available_packages: &BTreeSet<Package>,
) -> Vec<Package> {
    let mut processed = Vec::new();
    let mut packages: VecDeque<&Package> = VecDeque::from(packages);
    // On each iteration of the loop, we take the first element.
    // If it has missing dependencies, then we add those dependencies at the front of the list
    // (removing eventual occurrences later in the list) and proceed to the next iteration.
    // If it has no missing dependency, then we consume the package and remove it from the list.
    // In order to prevent infinite loops in the case of cyclic dependencies
    // (e.g. libintl depends on gcc-libs and libiconv, and libiconv depends on gcc-libs and libintl),
    // we keep snapshots of the package list, and if we are resetting the package list to a previous
    // state, then we need to consume the first package even if it has dependencies.
    let mut snapshots = BTreeSet::new();
    while !packages.is_empty() {
        let &package = packages.front().unwrap();
        let dependencies = package.dependencies.as_ref().and_then(|dependencies| {
            let dependencies: Vec<_> = dependencies
                .iter()
                .filter_map(|dependency| {
                    let dependency_package = dependency_name(dependency).and_then(|name| {
                        available_packages::latest_version(name, &available_packages)
                    });
                    if dependency_package.is_none() {
                        println!(
                            "{}",
                            Color::Red.paint(format!(
                                "Could not find {} dependency: {}",
                                package.name(),
                                dependency
                            ))
                        );
                    }
                    dependency_package
                })
                .filter(|&package| {
                    !installed_packages.contains(package) && !processed.contains(package)
                })
                .collect();
            if dependencies.is_empty() {
                None
            } else {
                Some(dependencies)
            }
        });
        let snapshot = packages
            .iter()
            .map(|&it| it.name())
            .collect::<Vec<_>>()
            .join(", ");
        // println!("{}", snapshot);
        if snapshots.insert(snapshot) && dependencies.is_some() {
            let mut dependencies = dependencies.unwrap();
            dependencies.sort_by_key(|&it| {
                -1 * it
                    .dependencies
                    .as_ref()
                    .map(|it| it.len() as i8)
                    .unwrap_or(0)
            });
            for dependency in dependencies {
                packages.retain(|&it| it != dependency);
                packages.push_front(dependency);
            }
        } else {
            let package = packages.pop_front().unwrap();
            if !processed.contains(package) {
                processed.push(package.clone());
            }
        }
    }
    // println!(
    //     "{}",
    //     processed
    //         .iter()
    //         .map(|it| it.name.to_string())
    //         .collect::<Vec<_>>()
    //         .join(", ")
    // );
    processed
}

fn dependency_name(name_with_optional_version: &str) -> Option<&str> {
    lazy_static! {
        static ref RE: Regex = Regex::new("[=>~#*]").unwrap();
    };
    (*RE)
        .split(name_with_optional_version)
        .into_iter()
        .next()
        .map(|it| match it {
            "sh" => "bash",
            it => it,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::packages::Packages;
    use std::path::PathBuf;

    lazy_static! {
        static ref DATA_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    }

    #[test]
    fn dependency_names() {
        let name = dependency_name("test");
        assert!(name.is_some());
        assert_eq!("test", name.unwrap());
        let name = dependency_name("package=1.0");
        assert!(name.is_some());
        assert_eq!("package", name.unwrap());
        let name = dependency_name("name>=1.0");
        assert!(name.is_some());
        assert_eq!("name", name.unwrap());
    }

    #[test]
    fn dependency_list_with_no_previous_installs() {
        let path = DATA_DIR.join("available_packages_file4.zst");
        let available_packages = Packages::get_packages_from_file(&path).unwrap();
        let package1 = available_packages::latest_version("package1", &available_packages);
        assert!(package1.is_some());
        let package1 = package1.unwrap();
        let package2 = available_packages::latest_version("package2", &available_packages);
        assert!(package2.is_some());
        let package2 = package2.unwrap();
        let empty = BTreeSet::new();
        let list = super::list(vec![package1], &empty, &available_packages);
        assert_eq!(2, list.len());
        let first = list.first().unwrap();
        let last = list.last().unwrap();
        assert_eq!(package2, first);
        assert_eq!(package1, last);
    }

    #[test]
    fn dependency_list_with_previous_installs() {
        let path = DATA_DIR.join("available_packages_file4.zst");
        let available_packages = Packages::get_packages_from_file(&path).unwrap();
        let package1 = available_packages::latest_version("package1", &available_packages);
        assert!(package1.is_some());
        let package1 = package1.unwrap();
        let package2 = available_packages::latest_version("package2", &available_packages);
        assert!(package2.is_some());
        let package2 = package2.unwrap();
        let mut installed_packages = BTreeSet::new();
        installed_packages.insert(package2.clone());
        let list = super::list(vec![package1], &installed_packages, &available_packages);
        assert_eq!(1, list.len());
        let first = list.first().unwrap();
        assert_eq!(package1, first);
    }
}
