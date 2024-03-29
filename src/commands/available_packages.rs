use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use ansi_term::Color;

use crate::commands::errors::Result;
use crate::commands::packages::{Package, Packages};
use crate::commands::repositories::{Repository, RepositoryVersion};
use crate::commands::utils::{file_was_updated_recently, ETag};
use std::fs;
use std::fs::File;
use std::process;

/// Checks if the cached list of available packages is up to date and updates it if necessary,
/// and then returns a set of packages.
pub fn get_packages(available_packages_file: &Path) -> BTreeSet<Package> {
    if file_was_updated_recently(available_packages_file) {
        match Packages::get_packages_from_file(available_packages_file) {
            Ok(packages) => return packages,
            Err(_) => {}
        }
    }
    let cache_versions = get_repository_versions_from_file(available_packages_file);
    let repositories_to_sync: Vec<&Repository> = Repository::enabled()
        .iter()
        .map(|it| *it)
        .filter(|&repository| match cache_versions.get(repository) {
            Some(cache_version) => match repository.remote_etag() {
                // sync if the etag have changed
                Ok(ref repo_etag) => repo_etag != &cache_version.etag,
                // we can't sync if we can't even get the etag
                Err(_) => false,
            },
            None => true, // sync if we don't have an etag (new repo or cache is missing)
        })
        .collect();
    if repositories_to_sync.is_empty() {
        // try to return the list from the cache
        if let Ok(packages) = Packages::get_packages_from_file(available_packages_file) {
            return packages;
        }
        // if the cache is missing/invalid, fetch the lists from the remote repositories,
        // then save the full list to the cache and return it
        get_packages_from_repositories(available_packages_file)
    } else {
        if repositories_to_sync.len() == Repository::enabled().len() {
            // we should be able to skip reading the cache because we need to sync everything
            // unless a repository sync fails
            let synced_repository_packages: Vec<_> = Repository::enabled()
                .iter()
                .map(|&repository| {
                    println!("Syncing {} repository", repository.name());
                    match repository.remote_packages() {
                        Ok(it) => Some(it),
                        Err(_) => {
                            println!(
                                "{}",
                                Color::Red.paint(format!(
                                    "Failed to sync {} repository.",
                                    repository.name()
                                ))
                            );
                            None
                        }
                    }
                })
                .collect();
            // if there were some failures we need to read the cache
            if synced_repository_packages.iter().any(|it| it.is_none()) {
                match Packages::get_packages_from_file(available_packages_file) {
                    Ok(packages) => {
                        let successfully_synced_packages: Vec<_> = synced_repository_packages
                            .into_iter()
                            .filter_map(|it| it)
                            .collect();
                        // if we didn't sync any repo successfully then we don't need to save
                        if successfully_synced_packages.is_empty() {
                            packages
                        } else {
                            let other_repositories: BTreeSet<&Repository> = Repository::enabled()
                                .iter()
                                .filter_map(|&repository| {
                                    if successfully_synced_packages
                                        .iter()
                                        .any(|it| it.version.repository == repository)
                                    {
                                        Repository::from(repository.name())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            let mut other_repository_packages: BTreeMap<&Repository, Packages> =
                                BTreeMap::new();
                            packages
                                .into_iter()
                                .filter(|it| other_repositories.contains(it.repository))
                                .for_each(|package| {
                                    other_repository_packages
                                        .remove(package.repository)
                                        .or_else(|| {
                                            cache_versions.get(package.repository).and_then(|it| {
                                                Some(Packages::create(it.clone(), Vec::new()))
                                            })
                                        });
                                });
                            let repository_packages: Vec<_> = successfully_synced_packages
                                .into_iter()
                                .chain(other_repository_packages.into_iter().map(|it| it.1))
                                .collect();
                            save_and_return_packages(available_packages_file, repository_packages)
                        }
                    }
                    Err(_) => {
                        // If we can't read the cache then we can't recover
                        println!("{}", Color::Red.paint("Aborting"));
                        process::exit(1);
                    }
                }
            }
            // if everything was successful then we just need to save
            else {
                let repository_packages = synced_repository_packages
                    .into_iter()
                    .filter_map(|it| it)
                    .collect();
                save_and_return_packages(available_packages_file, repository_packages)
            }
        } else {
            // try to read the list from the cache because we are not syncing all repos
            if let Ok(packages) = Packages::get_packages_from_file(available_packages_file) {
                // group the packages by repository
                let mut package_map: BTreeMap<&Repository, Vec<Package>> = BTreeMap::new();
                packages.into_iter().for_each(|it| {
                    let repository = it.repository;
                    let mut values = package_map.remove(it.repository).unwrap_or(Vec::new());
                    values.push(it);
                    package_map.insert(repository, values);
                });
                let mut full_list: Vec<Packages> = Vec::new();

                // map of Repository -> RepositoryVersion(etag, repo) for successful syncs
                let successful_syncs: BTreeMap<_, _> = Repository::enabled()
                    .iter()
                    .filter_map(|&repository| {
                        // fetch the packages if we need to sync this repository
                        // or filter the cache list if we don't need to sync or if the sync failed
                        let synced = if repositories_to_sync.contains(&repository) {
                            println!("Syncing {} repository", repository.name());
                            match repository.remote_packages() {
                                Ok(it) => Some(it),
                                Err(_) => {
                                    println!(
                                        "{}",
                                        Color::Red.paint(format!(
                                            "Failed to sync {} repository.",
                                            repository.name()
                                        ))
                                    );
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        match synced {
                            Some(it) => {
                                let version = &it.version.clone();
                                // add newly fetched packages
                                full_list.push(it);
                                Some((repository, version.clone()))
                            }
                            None => {
                                // add packages from the cache
                                if let Some(values) = package_map.remove(repository) {
                                    if let Some(cache_version) = cache_versions.get(repository) {
                                        full_list
                                            .push(Packages::create(cache_version.clone(), values))
                                    }
                                }
                                None
                            }
                        }
                    })
                    .collect();
                if successful_syncs.is_empty() {
                    // nothing to save
                    package_map.into_iter().flat_map(|it| it.1).collect()
                } else {
                    // save the new package list
                    save_and_return_packages(available_packages_file, full_list)
                }
            } else {
                // if the cache is missing/invalid, fetch the lists from the remote repositories,
                // then save the full list to the cache and return it
                get_packages_from_repositories(available_packages_file)
            }
        }
    }
}

pub fn latest_version<'a>(name: &str, packages: &'a BTreeSet<Package>) -> Option<&'a Package> {
    packages
        .iter()
        .filter(|&it| it.matches(name))
        .max_by_key(|&it| &it.version)
}

fn save_and_return_packages(
    available_packages_file: &Path,
    repository_packages: Vec<Packages>,
) -> BTreeSet<Package> {
    match save_packages(available_packages_file, &repository_packages) {
        Ok(_) => repository_packages
            .into_iter()
            .flat_map(|it| it.list)
            .collect(),
        Err(_) => {
            println!("{}", Color::Red.paint("Failed to save cache."));
            repository_packages
                .into_iter()
                .flat_map(|it| it.list)
                .collect()
        }
    }
}

fn save_packages(
    available_packages_file: &Path,
    repository_packages: &Vec<Packages>,
) -> Result<()> {
    // build content before opening the file to minimize the time the file is only partially written
    let mut data = Vec::with_capacity(131_072);
    let mut encoder = zstd::Encoder::new(&mut data, zstd::DEFAULT_COMPRESSION_LEVEL)?;
    // header: repo1_name etag1 repo2_name etag2 ...
    let mut header = repository_packages
        .iter()
        .map(|it| format!("{} {}", it.version.repository.name(), &it.version.etag))
        .collect::<Vec<_>>()
        .join(" ");
    header += "\n";
    encoder.write_all(&header.as_bytes())?;
    // for each repo, add 1 line for each package
    for repo in repository_packages {
        for package in &repo.list {
            let mut line = String::from(package);
            line += "\n";
            encoder.write_all(&line.as_bytes())?;
        }
    }
    encoder.finish()?;
    fs::write(available_packages_file, &data)?;
    Ok(())
}

fn get_packages_from_repositories(available_packages_file: &Path) -> BTreeSet<Package> {
    let repository_packages: Vec<_> = Repository::enabled()
        .iter()
        .map(|&repository| {
            println!("Syncing {} repository", repository.name());
            match repository.remote_packages() {
                Ok(it) => it,
                Err(_) => {
                    println!(
                        "{}",
                        Color::Red.paint(format!(
                            "Failed to sync {} repository.\nAborting.",
                            repository.name()
                        ))
                    );
                    process::exit(1);
                }
            }
        })
        .collect();
    match save_packages(available_packages_file, &repository_packages) {
        Ok(()) => repository_packages
            .into_iter()
            .flat_map(|it| it.list)
            .collect(),
        Err(_) => {
            println!(
                "{}",
                Color::Red.paint("Failed to create {} package cache.\nAborting.")
            );
            process::exit(1);
        }
    }
}

fn get_repository_versions_from_file(
    available_packages_file: &Path,
) -> BTreeMap<&Repository, RepositoryVersion> {
    File::open(available_packages_file)
        .and_then(|file| zstd::Decoder::new(file))
        .map(|decoder| BufReader::new(decoder))
        .ok()
        .and_then(|reader| {
            if let Some(Ok(first_line)) = reader.lines().next() {
                let cols: Vec<&str> = first_line.split(' ').collect();
                cols.chunks(2)
                    .map(|it| {
                        Repository::from(&it[0]).map(|repo| {
                            (
                                repo,
                                RepositoryVersion {
                                    repository: repo,
                                    etag: ETag::from(it[1]),
                                },
                            )
                        })
                    })
                    .collect()
            } else {
                None
            }
        })
        .unwrap_or(BTreeMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use std::path::PathBuf;

    lazy_static! {
        static ref DATA_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    }

    #[test]
    fn read_packages_from_file() {
        read_packages_from_file_at("available_packages_file1.zst")
    }

    fn read_packages_from_file_at(filename: &str) {
        let packages = Packages::get_packages_from_file(&DATA_DIR.join(filename)).unwrap();
        assert_eq!(packages.len(), 5);
        assert!(packages
            .get(&Package::try_from("msys\tpackage1\t1.0").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("msys\tpackage2\t1.0\tzst\tx86_64").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("msys\tpackage3\t1.0.1\tzst\tany").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("mingw64\tpackage4\t3.1").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("mingw64\tpackage5\t3.1.2").unwrap())
            .is_some());
    }

    #[test]
    fn read_repository_versions_from_file() {
        let path = DATA_DIR.join("non_existing.zst");
        let versions = get_repository_versions_from_file(&path);
        assert!(versions.is_empty());
        let path = DATA_DIR.join("available_packages_file2.zst");
        let versions = get_repository_versions_from_file(&path);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions.get(&Repository::Mingw64).unwrap().etag.value, "3");
        read_repository_versions_from_file_at("available_packages_file1.zst");
    }

    fn read_repository_versions_from_file_at(filename: &str) {
        let path = DATA_DIR.join(filename);
        let versions = get_repository_versions_from_file(&path);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions.get(&Repository::Msys).unwrap().etag.value, "1");
        assert_eq!(versions.get(&Repository::Mingw64).unwrap().etag.value, "2");
    }

    #[test]
    fn write_packages_to_file() {
        rm_rf::ensure_removed(&DATA_DIR.join("tmp")).unwrap();
        let path = DATA_DIR.join("available_packages_file1.zst");
        let packages = Packages::get_packages_from_file(&path).unwrap();
        let mut msys_packages = Vec::new();
        let mut mingw64_packages = Vec::new();
        packages.into_iter().for_each(|it| match it.repository {
            &Repository::Msys => msys_packages.push(it),
            &Repository::Mingw64 => mingw64_packages.push(it),
            _ => panic!(),
        });
        let packages = vec![
            Packages {
                version: RepositoryVersion {
                    repository: &Repository::Msys,
                    etag: ETag {
                        value: "1".to_string(),
                    },
                },
                list: msys_packages,
            },
            Packages {
                version: RepositoryVersion {
                    repository: &Repository::Mingw64,
                    etag: ETag {
                        value: "2".to_string(),
                    },
                },
                list: mingw64_packages,
            },
        ];
        assert!(save_packages(&DATA_DIR.join("tmp"), &packages).is_ok());
        let _ = Packages::get_packages_from_file(&DATA_DIR.join("tmp")).unwrap();
        read_repository_versions_from_file_at("tmp");
        read_packages_from_file_at("tmp");
    }

    #[test]
    fn latest_version() {
        let path = DATA_DIR.join("available_packages_file3.zst");
        let packages = Packages::get_packages_from_file(&path).unwrap();
        let latest = super::latest_version("name", &packages);
        assert!(latest.is_some());
        assert_eq!("1.2", latest.unwrap().version.as_str());
        let latest = super::latest_version("other", &packages);
        assert!(latest.is_some());
        assert_eq!("0.9", latest.unwrap().version.as_str());
        let latest = super::latest_version("not_there", &packages);
        assert!(latest.is_none());
    }
}
