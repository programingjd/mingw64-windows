use crate::commands::dependencies;
use crate::commands::errors::{Error, Result};
use crate::commands::packages::Package;
use crate::commands::utils::YesNoAnswer::YES;
use crate::commands::{available_packages, utils};
use crate::commands::{installed_packages, paths};
use ansi_term::Color;
use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::path::Path;
use std::{fs, process};
use tar::EntryType;

pub fn install(root_directory_path: &Path, packages: BTreeSet<Package>) {
    let installed_packages_file_path = paths::get_installed_packages_file_path(root_directory_path);
    let installed_packages = installed_packages::get_packages(&installed_packages_file_path);
    let installed_packages: BTreeSet<_> = installed_packages.iter().collect();
    let available_packages_file_path = paths::get_available_packages_file_path(root_directory_path);
    let available_packages = available_packages::get_packages(&available_packages_file_path);

    // add bash and coreutils if they aren't installed already, as they are needed to run
    // some of the packages post-install scripts.
    // also remove packages that are already installed (with the same version).
    let packages: Vec<_> = if installed_packages
        .iter()
        .find(|&it| &it.name == "bash")
        .is_none()
    {
        let bash = available_packages::latest_version("bash", &available_packages);
        if bash.is_none() {
            println!(
                "{}",
                Color::Red.paint("Could not find bash package. Aborting.")
            );
            process::exit(1);
        }
        let bash = bash.unwrap();
        // if bash is missing, then coreutils is missing too.
        let coreutils = available_packages::latest_version("coreutils", &available_packages);
        if coreutils.is_none() {
            println!(
                "{}",
                Color::Red.paint("Could not find coreutils package. Aborting.")
            );
            process::exit(1);
        }
        let coreutils = coreutils.unwrap();
        vec![bash, coreutils]
            .into_iter()
            .chain(packages.iter().filter(|&it| {
                !installed_packages.contains(it) && it.name != "bash" && it.name != "coreutils"
            }))
            .collect()
    } else if installed_packages
        .iter()
        .find(|&it| &it.name == "coreutils")
        .is_none()
    {
        let coreutils = available_packages::latest_version("coreutils", &available_packages);
        if coreutils.is_none() {
            println!(
                "{}",
                Color::Red.paint("Could not find coreutils package. Aborting.")
            );
            process::exit(1);
        }
        let coreutils = coreutils.unwrap();
        vec![coreutils]
            .into_iter()
            .chain(
                packages
                    .iter()
                    .filter(|&it| !installed_packages.contains(it) && it.name != "coreutils"),
            )
            .collect()
    } else {
        packages
            .iter()
            .filter(|&it| !installed_packages.contains(it))
            .collect()
    };

    for package in dependencies::list(packages, installed_packages, &available_packages) {
        if install_package(root_directory_path, &package).is_err() {
            println!(
                "{}",
                Color::Red.paint(format!("Failed to install {}. Aborting.", &package.name))
            );
            process::exit(1);
        }
    }

    // let mut snapshots = BTreeSet::new();
    // while !packages.is_empty() {
    //     let &package = packages.front().unwrap();
    //     let dependencies = package.dependencies.as_ref().and_then(|dependencies| {
    //         let dependencies: Vec<_> = dependencies
    //             .iter()
    //             .filter_map(|dependency| {
    //                 let dependency_package = dependency_name(dependency).and_then(|name| {
    //                     available_packages::latest_version(name, &available_packages)
    //                 });
    //                 if dependency_package.is_none() {
    //                     println!(
    //                         "{}",
    //                         Color::Red.paint(format!(
    //                             "Could not find {} dependency: {}",
    //                             &package.name, dependency
    //                         ))
    //                     );
    //                 }
    //                 dependency_package
    //             })
    //             .filter(|&package| !installed_packages.contains(package))
    //             .collect();
    //         if dependencies.is_empty() {
    //             None
    //         } else {
    //             Some(dependencies)
    //         }
    //     });
    //     let snapshot = packages
    //         .iter()
    //         .map(|&it| it.name.as_str())
    //         .collect::<Vec<_>>()
    //         .join(", ");
    //     println!("{}", snapshot);
    //     if snapshots.insert(snapshot) && dependencies.is_some() {
    //         let mut dependencies = dependencies.unwrap();
    //         dependencies.sort_by_key(|&it| {
    //             -1 * it
    //                 .dependencies
    //                 .as_ref()
    //                 .map(|it| it.len() as i8)
    //                 .unwrap_or(0)
    //         });
    //         for dependency in dependencies {
    //             packages.retain(|&it| it != dependency);
    //             packages.push_front(dependency);
    //         }
    //     } else {
    //         match install_package(root_directory_path, package) {
    //             Ok(_) => {
    //                 installed_packages.insert(packages.pop_front().unwrap());
    //             }
    //             Err(_) => {
    //                 println!(
    //                     "{}",
    //                     Color::Red.paint(format!("Failed to install {}. Aborting.", &package.name))
    //                 );
    //                 process::exit(1);
    //             }
    //         }
    //     }
    // }
}

pub fn update(root_directory_path: &Path, packages: BTreeSet<Package>) {
    todo!()
}

fn install_package(root_directory_path: &Path, package: &Package) -> Result<()> {
    println!("{}", Color::Purple.paint(&package.name));
    // first update the pending installation file so that we retry the installation if we crash or
    // the program is interrupted.
    let pending_installation_file_path =
        paths::get_pending_installation_file_path(root_directory_path);
    fs::write(
        &pending_installation_file_path,
        String::from(package).as_str(),
    )?;
    let bytes = download_package_archive(package)?;
    let compression = package.compression.unwrap();
    let bytes = match compression.decompress(bytes.as_slice()) {
        Ok(bytes) => bytes,
        Err(err) => {
            println!(
                "{}",
                Color::Red.paint(format!(
                    "Failed to decompress {} archive for {}",
                    compression.extension(),
                    &package.name
                ))
            );
            return Err(err);
        }
    };
    extract_package(root_directory_path, bytes.as_slice())?;
    // update the installed packages file
    installed_packages::append_package(root_directory_path, package)?;
    // remove the pending installation file
    rm_rf::remove(&pending_installation_file_path).map_err(|_| Error::RemoveError)?;
    Ok(())
}

fn download_package_archive(package: &Package) -> Result<Vec<u8>> {
    let url = package.url.as_ref().unwrap();
    match utils::download(url) {
        Ok(response) => Ok(response.body),
        Err(err) => {
            println!(
                "{}",
                Color::Red.paint(format!(
                    "Failed to download archive for {} from {}",
                    &package.name, url
                ))
            );
            Err(err)
        }
    }
}

fn extract_package(root_directory_path: &Path, uncompressed_package_archive: &[u8]) -> Result<()> {
    let bash_env_path = root_directory_path
        .join("usr")
        .join("bin")
        .to_string_lossy()
        .to_string();
    match tar::Archive::new(uncompressed_package_archive).entries() {
        Ok(entries) => {
            entries.filter_map(|it| it.ok()).for_each(|mut entry| {
                match entry.path() {
                    Ok(name) => {
                        if name.is_relative() {
                            match name.to_string_lossy().borrow() {
                                ".BUILDINFO" => {}
                                ".MTREE" => {}
                                ".PKGINFO" => {}
                                ".INSTALL" => {
                                    let path = root_directory_path.join(name);
                                    entry.unpack(&path).unwrap();
                                    match std::process::Command::new(
                                        &root_directory_path.join("usr").join("bin").join("bash.exe"),
                                    )
                                    .current_dir(root_directory_path)
                                    .arg(".INSTALL")
                                    .env("PATH", &bash_env_path)
                                    .output()
                                    {
                                        Ok(output) => {
                                            if !output.stderr.is_empty() {
                                                println!(
                                                    "{}",
                                                    Color::Red.paint(String::from_utf8_lossy(
                                                        &output.stderr
                                                    ))
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            println!(
                                                "{}",
                                                Color::Red.paint("Failed to run install script")
                                            );
                                            println!("{:?}", err);
                                        }
                                    };
                                    std::fs::remove_file(&path).unwrap();
                                }
                                name => {
                                    if !name.contains("..") {
                                        // println!("{}", &name.to_string());
                                        let path = root_directory_path.join(name);
                                        path.parent().and_then(|parent| {
                                            std::fs::create_dir_all(parent).ok()
                                        });
                                        if match entry.header().entry_type() {
                                            EntryType::Directory => fs::create_dir_all(&path).ok(),
                                            EntryType::Link | EntryType::Symlink => {
                                                entry
                                                    .link_name()
                                                    .ok()
                                                    .and_then(|it| it)
                                                    .and_then(|it| {
                                                        junction::create(
                                                            path.join(it.to_str().unwrap()),
                                                            &path
                                                        ).ok()
                                                    })
                                            },
                                            EntryType::Regular => {
                                                rm_rf::ensure_removed(&path).ok()
                                                    .and_then(|_| entry.unpack(&path).map(|_| ()).ok())
                                            }
                                            it => {
                                                println!(
                                                    "{}",
                                                    Color::Red.paint(&format!(
                                                        "Skipping unsupported entry {} of type {:?}",
                                                        path.strip_prefix(root_directory_path)
                                                            .unwrap()
                                                            .to_string_lossy(),
                                                        it
                                                    ))
                                                );
                                                Some(())
                                            }
                                        }.is_none() {
                                            println!(
                                                "{}",
                                                Color::Red.paint(&format!(
                                                    "Failed to extract {}",
                                                    path.strip_prefix(root_directory_path)
                                                        .unwrap()
                                                        .to_string_lossy()
                                                ))
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => println!("{}", &Color::Red.paint("Invalid path in tar archive")),
                };
            });
        }
        Err(_) => return Err(Error::DecompressionError),
    }
    Ok(())
}

pub fn check_for_pending_installation(root_directory_path: &Path, no_prompt: bool) {
    let pending_installation_file_path =
        paths::get_pending_installation_file_path(root_directory_path);
    if pending_installation_file_path.exists() {
        if let Ok(s) = fs::read_to_string(&pending_installation_file_path) {
            if !s.is_empty() {
                match Package::try_from(s.as_str()) {
                    Ok(package) => {
                        println!(
                            "Installation of {} did not finish successfully.",
                            Color::Purple.paint(&package.name)
                        );
                        let answer = utils::yes_or_no("Retry?", YES, no_prompt, Some("Retrying."));
                        match answer {
                            YES => {
                                let mut packages = BTreeSet::new();
                                packages.insert(package);
                                install(root_directory_path, packages);
                            }
                            _ => {
                                let _ = rm_rf::remove(&pending_installation_file_path);
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
}
