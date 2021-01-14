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
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::{fs, process};
use tar::EntryType;

const INSTALL: &'static str = "install";
const UPDATE: &'static str = "update";

pub fn install(root_directory_path: &Path, packages: BTreeSet<Package>) {
    let installed_packages_file_path = paths::get_installed_packages_file_path(root_directory_path);
    let mut installed_packages = installed_packages::get_packages(&installed_packages_file_path);
    let available_packages_file_path = paths::get_available_packages_file_path(root_directory_path);
    let available_packages = available_packages::get_packages(&available_packages_file_path);

    // We need bash, info, and coreutils to run post-install scripts.
    // However, info and coreutils and/or their dependencies have post-install scripts.
    // Therefore, we first install bash, and then install info and coreutils with
    // setup=true to skip running the scripts and flagging them as installed.
    // Then we run the installation of info and coreutils as normal.

    let bash = missing_packages(vec!["bash"], &installed_packages, &available_packages);
    if !bash.is_empty() {
        for package in dependencies::list(bash, &installed_packages, &available_packages) {
            if install_package(root_directory_path, &package, false).is_err() {
                println!(
                    "{}",
                    Color::Red.paint(format!("Failed to install {}. Aborting.", package.name()))
                );
                process::exit(1);
            }
            installed_packages.insert(package);
        }
    }

    let info_coreutils = missing_packages(
        vec!["info", "coreutils"],
        &installed_packages,
        &available_packages,
    );
    if !info_coreutils.is_empty() {
        for package in dependencies::list(
            info_coreutils.clone(),
            &installed_packages,
            &available_packages,
        ) {
            if install_package(root_directory_path, &package, true).is_err() {
                println!(
                    "{}",
                    Color::Red.paint(format!("Failed to install {}. Aborting.", package.name()))
                );
                process::exit(1);
            }
        }
        for package in dependencies::list(info_coreutils, &installed_packages, &available_packages)
        {
            if install_package(root_directory_path, &package, false).is_err() {
                println!(
                    "{}",
                    Color::Red.paint(format!("Failed to install {}. Aborting.", package.name()))
                );
                process::exit(1);
            }
            installed_packages.insert(package);
        }
    }

    for package in dependencies::list(
        packages.iter().collect(),
        &installed_packages,
        &available_packages,
    ) {
        if install_package(root_directory_path, &package, false).is_err() {
            println!(
                "{}",
                Color::Red.paint(format!("Failed to install {}. Aborting.", package.name()))
            );
            process::exit(1);
        }
    }
}

pub fn update(root_directory_path: &Path, packages: BTreeSet<Package>) {
    let installed_packages_file_path = paths::get_installed_packages_file_path(root_directory_path);
    let installed_packages = installed_packages::get_packages(&installed_packages_file_path);
    for package in packages {
        if !installed_packages.contains(&package) {
            if update_package(root_directory_path, &package).is_err() {
                println!(
                    "{}",
                    Color::Red.paint(format!("Failed to update {}. Aborting.", package.name()))
                );
                process::exit(1);
            }
        }
    }
}

fn missing_packages<'a>(
    packages: Vec<&str>,
    installed_packages: &BTreeSet<Package>,
    available_packages: &'a BTreeSet<Package>,
) -> Vec<&'a Package> {
    packages
        .into_iter()
        .filter_map(
            |name| match installed_packages.iter().find(|&it| it.matches(name)) {
                Some(_) => None,
                None => match available_packages::latest_version(name, available_packages) {
                    Some(it) => Some(it),
                    None => {
                        println!(
                            "{}",
                            Color::Red
                                .paint(&format!("Could not find {} package. Aborting.", name))
                        );
                        process::exit(1);
                    }
                },
            },
        )
        .collect()
}

fn update_package(root_directory_path: &Path, package: &Package) -> Result<()> {
    println!(
        "{} {}",
        Color::Purple.paint(package.name()),
        package.version
    );
    let pending_installation_file_path =
        paths::get_pending_installation_file_path(root_directory_path);
    // first update the pending installation file so that we can retry
    // the update if we crash or the program is interrupted.
    fs::write(
        &pending_installation_file_path,
        format!("{}\n{}", UPDATE, String::from(package)).as_str(),
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
                    package.name()
                ))
            );
            return Err(err);
        }
    };
    extract_package(root_directory_path, bytes.as_slice(), false)?;
    // update the installed packages file
    installed_packages::replace_package(root_directory_path, package)?;
    // remove the pending installation file
    rm_rf::remove(&pending_installation_file_path).map_err(|_| Error::RemoveError)?;
    Ok(())
}

// We need bash, info and coreutils to run install scripts,
// but coreutils and some of its dependencies have install scripts of their own.
// Therefore, we need a first step that installs those packages first
// without running the install scripts (setup arg to true)
// and without flagging those packages as installed.
// After that, we can reinstall those packages as normal.
fn install_package(root_directory_path: &Path, package: &Package, setup: bool) -> Result<()> {
    if !setup {
        println!(
            "{} {}",
            Color::Purple.paint(package.name()),
            package.version
        );
    }
    let pending_installation_file_path =
        paths::get_pending_installation_file_path(root_directory_path);
    if !setup {
        // first update the pending installation file so that we can retry
        // the installation if we crash or the program is interrupted.
        fs::write(
            &pending_installation_file_path,
            format!("{}\n{}", INSTALL, String::from(package)).as_str(),
        )?;
    }
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
                    package.name()
                ))
            );
            return Err(err);
        }
    };
    extract_package(root_directory_path, bytes.as_slice(), setup)?;
    if !setup {
        // update the installed packages file
        installed_packages::append_package(root_directory_path, package)?;
        // remove the pending installation file
        rm_rf::remove(&pending_installation_file_path).map_err(|_| Error::RemoveError)?;
    }
    Ok(())
}

fn download_package_archive(package: &Package) -> Result<Vec<u8>> {
    let url = package.url().unwrap();
    match utils::download(&url) {
        Ok(response) => Ok(response.body),
        Err(err) => {
            println!(
                "{}",
                Color::Red.paint(format!(
                    "Failed to download archive for {} from {}",
                    package.name(),
                    url
                ))
            );
            Err(err)
        }
    }
}

fn extract_package(
    root_directory_path: &Path,
    uncompressed_package_archive: &[u8],
    setup: bool,
) -> Result<()> {
    // two steps, regular files first, and then links
    // regular files
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
                                    if !setup {
                                        // install script that we will run later
                                        let path = root_directory_path.join(name);
                                        entry.unpack(&path).unwrap();
                                    }
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
                                            EntryType::Link | EntryType::Symlink => Some(()),
                                            EntryType::Regular => rm_rf::ensure_removed(&path)
                                                .ok()
                                                .and_then(|_| entry.unpack(&path).map(|_| ()).ok()),
                                            it => {
                                                println!(
                                                    "{}",
                                                    Color::Red.paint(&format!(
                                                        "Skipping unsupported {:?} entry {}",
                                                        &entry_type_name(&it),
                                                        path.strip_prefix(root_directory_path)
                                                            .unwrap()
                                                            .to_string_lossy()
                                                    ))
                                                );
                                                Some(())
                                            }
                                        }
                                        .is_none()
                                        {
                                            if !setup {
                                                println!(
                                                    "{}",
                                                    Color::Red.paint(&format!(
                                                        "Failed to create {} {}",
                                                        &entry_type_name(
                                                            &entry.header().entry_type()
                                                        ),
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
                    }
                    Err(_) => println!("{}", &Color::Red.paint("Invalid path in tar archive")),
                };
            });
        }
        Err(_) => return Err(Error::DecompressionError),
    }
    // links
    match tar::Archive::new(uncompressed_package_archive).entries() {
        Ok(entries) => {
            entries.filter_map(|it| it.ok()).for_each(|entry| {
                match entry.path() {
                    Ok(name) => {
                        if name.is_relative() {
                            match name.to_string_lossy().borrow() {
                                ".BUILDINFO" => {}
                                ".MTREE" => {}
                                ".PKGINFO" => {}
                                ".INSTALL" => {}
                                name => {
                                    if !name.contains("..") {
                                        let path = root_directory_path.join(name);
                                        if match entry.header().entry_type() {
                                            EntryType::Link | EntryType::Symlink => {
                                                rm_rf::ensure_removed(&path).ok().and_then(|_| {
                                                    entry
                                                        .link_name()
                                                        .ok()
                                                        .and_then(|it| it)
                                                        .and_then(|it| {
                                                            let target = it.to_str().unwrap();
                                                            let target = if target.starts_with('/')
                                                            {
                                                                root_directory_path.join(
                                                                    target.replacen("/", "", 1),
                                                                )
                                                            } else {
                                                                root_directory_path.join(target)
                                                            };
                                                            if target.is_dir() {
                                                                junction::create(&target, &path)
                                                                    .ok()
                                                            } else if target.is_file() {
                                                                fs::hard_link(&target, &path).ok()
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                })
                                            }
                                            _ => Some(()),
                                        }
                                        .is_none()
                                        {
                                            if !setup {
                                                println!(
                                                    "{}",
                                                    Color::Red.paint(&format!(
                                                        "Failed to create {} /{}",
                                                        &entry_type_name(
                                                            &entry.header().entry_type()
                                                        ),
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
                    }
                    Err(_) => println!("{}", &Color::Red.paint("Invalid path in tar archive")),
                };
            });
        }
        Err(_) => return Err(Error::DecompressionError),
    }
    if !setup {
        // run the install script
        let path = root_directory_path.join(".INSTALL");
        if path.exists() {
            let bash_env_path = root_directory_path
                .join("usr")
                .join("bin")
                .to_string_lossy()
                .to_string();
            match std::process::Command::new(
                &root_directory_path.join("usr").join("bin").join("bash.exe"),
            )
            .current_dir(root_directory_path)
            .args(&["-c", "source /.INSTALL && (declare -F -f post_install && post_install) || (declare -F -f post_upgrade && post_upgrade)"])    
            .env("PATH", &bash_env_path)
            .output()
            {
                Ok(output) => {
                    if !output.stderr.is_empty() {
                        println!(
                            "{}",
                            Color::Red.paint(String::from_utf8_lossy(&output.stderr))
                        );
                    }
                }
                Err(err) => {
                    println!("{}", Color::Red.paint("Failed to run install script"));
                    println!("{:?}", err);
                }
            };
            std::fs::remove_file(&path).unwrap();
        }
    }
    Ok(())
}

fn entry_type_name(entry_type: &EntryType) -> String {
    match entry_type {
        EntryType::Regular => "file".to_string(),
        EntryType::Directory => "directory".to_string(),
        EntryType::Symlink => "symlink".to_string(),
        EntryType::Link => "link".to_string(),
        _ => format!("{:?}", entry_type).to_ascii_lowercase(),
    }
}

pub fn check_for_pending_installation(root_directory_path: &Path, no_prompt: bool) {
    let pending_installation_file_path =
        paths::get_pending_installation_file_path(root_directory_path);
    if pending_installation_file_path.exists() {
        if let Ok(mut lines) =
            File::open(&pending_installation_file_path).map(|file| BufReader::new(file).lines())
        {
            let command = lines.next().and_then(|it| it.ok());
            let package = lines
                .next()
                .and_then(|it| it.ok())
                .and_then(|ref it| Package::try_from(it.as_str()).ok());
            if package.is_some() && command.is_some() {
                let command = command.unwrap();
                let package = package.unwrap();
                match command.as_str() {
                    INSTALL => {
                        println!(
                            "Installation of {} did not finish successfully.",
                            Color::Purple.paint(package.name())
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
                    UPDATE => {
                        println!(
                            "Update of {} did not finish successfully.",
                            Color::Purple.paint(package.name())
                        );
                        let answer = utils::yes_or_no("Retry?", YES, no_prompt, Some("Retrying."));
                        match answer {
                            YES => {
                                let mut packages = BTreeSet::new();
                                packages.insert(package);
                                update(root_directory_path, packages);
                            }
                            _ => {
                                let _ = rm_rf::remove(&pending_installation_file_path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
