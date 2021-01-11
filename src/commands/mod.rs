use std::path::{Path, PathBuf};

use crate::commands::packages::Package;
use crate::commands::utils::YesNoAnswer::{NO, YES};
use ansi_term::{ANSIString, Color};
use core::cmp::Ordering;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::process::exit;

mod available_packages;
mod errors;
mod installed_packages;
mod installer;
mod packages;
mod paths;
mod repositories;
mod utils;

/// Automatically selects the current directory if ./var/lib/packages/installed exists, otherwise
/// asks the user.
pub fn root_directory() -> PathBuf {
    let root_directory_path = if let Ok(current_directory) = env::current_dir() {
        if paths::get_installed_packages_file_path(&current_directory).exists() {
            current_directory
        } else {
            let fs = current_directory.join("fs");
            if paths::get_installed_packages_file_path(&fs).exists() {
                fs
            } else {
                prompt_for_directory(Some(&current_directory))
            }
        }
    } else {
        prompt_for_directory(None)
    };
    paths::create_directory_structure(&root_directory_path);
    root_directory_path
}

pub fn list_installed_packages(root_directory_path: &Path, packages: BTreeSet<&str>) {
    let path = paths::get_installed_packages_file_path(root_directory_path);
    if path.exists() {
        let installed_packages = installed_packages::get_packages(&path);
        if installed_packages.len() > 0 {
            packages
                .iter()
                .filter_map(|name| {
                    installed_packages
                        .iter()
                        .find(|&it| &it.name == name)
                        .map(|package| {
                            format!(
                                "{} {}",
                                Color::Purple.paint(&package.name),
                                &package.version
                            )
                        })
                })
                .for_each(|it| println!("{}", it));
        }
    }
}

pub fn list_all_installed_packages(root_directory_path: &Path) {
    let path = paths::get_installed_packages_file_path(root_directory_path);
    if path.exists() {
        let packages = installed_packages::get_packages(&path);
        if packages.len() == 0 {
            println!("No package installed.")
        } else {
            packages
                .iter()
                .map(|package| {
                    format!(
                        "{} {}",
                        Color::Purple.paint(&package.name),
                        &package.version
                    )
                })
                .for_each(|it| println!("{}", it));
        }
    } else {
        println!("No package installed.")
    }
}

pub fn search_available_packages(root_directory_path: &Path, terms: BTreeSet<&str>) {
    let path = paths::get_available_packages_file_path(root_directory_path);
    let packages = available_packages::get_packages(&path);
    let mut results: Vec<_> = packages
        .iter()
        .filter_map(|package| {
            let name = &package.name;
            let score: u16 = terms
                .iter()
                .filter_map(|&term| {
                    if name.starts_with(term) {
                        Some(4u16)
                    } else if name.starts_with(format!("lib{}", term).as_str()) {
                        Some(2u16)
                    } else if name.contains(term) {
                        Some(1u16)
                    } else {
                        None
                    }
                })
                .sum();
            if score == 0 {
                None
            } else {
                Some((score, package))
            }
        })
        .collect();
    results.sort_by(|a, b| match &a.0.cmp(&b.0) {
        Ordering::Equal => a.1.cmp(b.1).reverse(),
        &it => it.reverse(),
    });
    results.iter().for_each(|it| {
        let package = it.1;
        let mut name = ANSIString::from(&package.name);
        let version = &package.version;
        for &term in &terms {
            let replacement = Color::Green.paint(term);
            let pieces: Vec<_> = name.split(term).collect();
            let mut iter = pieces.iter();
            let mut str = iter.next().map(|it| it.clone()).unwrap_or("").to_string();
            while let Some(&cur) = iter.next() {
                str = format!("{}{}{}", str, replacement, cur)
            }
            name = ANSIString::from(str);
        }
        // println!("{}", Color::Purple.paint(format!("{} {}", &name, version)));
        println!("{} {}", &name, version);
    });
}

pub fn install_packages(root_directory_path: &Path, package_names: BTreeSet<&str>) {
    installer::check_for_pending_installation(root_directory_path);
    let path = paths::get_available_packages_file_path(root_directory_path);
    let available_packages = available_packages::get_packages(&path);
    let mut not_found: Vec<&str> = Vec::new();
    let mut packages: BTreeSet<&Package> = BTreeSet::new();
    for name in package_names {
        match available_packages::latest_version(name, &available_packages) {
            Some(package) => {
                packages.insert(package);
            }
            None => not_found.push(name),
        }
    }
    match not_found.len() {
        0 => {}
        1 => {
            println!(
                "{}",
                Color::Red.paint(format!(
                    "Could not find package: {}",
                    not_found.first().unwrap()
                ))
            );
            if packages.is_empty() || utils::yes_or_no("Abort installation?", NO) == YES {
                return;
            }
        }
        n => {
            println!(
                "{}",
                &Color::Red.paint(format!(
                    "Could not find the following packages: {}",
                    not_found.join(", ")
                ))
            );
            if packages.len() <= n || utils::yes_or_no("Abort installation?", NO) == YES {
                return;
            }
        }
    }
    installer::install(root_directory_path, packages);
}

fn prompt_for_directory(default: Option<&Path>) -> PathBuf {
    let selection = utils::text_input(
        "Installation directory:",
        default.and_then(|it| it.to_str()),
    );
    let path = Path::new(&selection);
    if path.exists() {
        path.to_path_buf()
    } else {
        match utils::yes_or_no("Directory doesn't exist. Create it?", YES) {
            YES => match fs::create_dir_all(path) {
                Ok(_) => path.to_path_buf(),
                Err(_) => {
                    println!(
                        "{}",
                        Color::Red.paint("Failed to create the directory structure.")
                    );
                    exit(1)
                }
            },
            NO => exit(0),
        }
    }
}
