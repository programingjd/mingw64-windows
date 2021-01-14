use std::path::{Path, PathBuf};

use crate::commands::packages::Package;
use crate::commands::utils::YesNoAnswer::{NO, YES};
use ansi_term::{ANSIString, Color};
use core::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::process::exit;
use std::{env, process};

mod available_packages;
mod dependencies;
mod errors;
mod installed_packages;
mod installer;
mod packages;
mod paths;
mod repositories;
mod utils;

/// Automatically selects the current directory if ./var/lib/packages/installed exists, otherwise
/// asks the user.
pub fn root_directory(no_prompt: bool) -> PathBuf {
    let root_directory_path = if let Ok(current_directory_path) = env::current_dir() {
        if paths::get_available_packages_file_path(&current_directory_path).exists() {
            current_directory_path
        } else {
            let fs = current_directory_path.join("fs");
            if paths::get_available_packages_file_path(&fs).exists() {
                fs
            } else {
                if no_prompt {
                    println!(
                        "{}",
                        Color::Red.paint("Installation root directory not found.")
                    );
                    println!("{}", Color::Cyan.paint("Selecting the current directory."));
                    current_directory_path
                } else {
                    prompt_for_directory(Some(&current_directory_path))
                }
            }
        }
    } else {
        if no_prompt {
            println!(
                "{}",
                Color::Red.paint("Could not find installation root directory. Aborting.")
            );
            process::exit(1);
        }
        prompt_for_directory(None)
    };
    paths::create_directory_structure(&root_directory_path, no_prompt);
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
                        .find(|&it| it.matches(name))
                        .map(|package| {
                            format!(
                                "{} {}",
                                Color::Purple.paint(package.name()),
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
                        Color::Purple.paint(package.name()),
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
            let name = package.name();
            let score: u16 = terms
                .iter()
                .filter_map(|&term| {
                    if name == term {
                        Some(8u16)
                    } else if name.starts_with(term) {
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
        let mut name = ANSIString::from(package.name());
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

pub fn list_dependencies(
    root_directory_path: &Path,
    package_names: BTreeSet<&str>,
    no_prompt: bool,
) {
    let available_packages_file_path = paths::get_available_packages_file_path(root_directory_path);
    let available_packages = available_packages::get_packages(&available_packages_file_path);
    let empty = BTreeSet::new();
    let mut results = dependencies::list(
        get_packages(root_directory_path, package_names, no_prompt)
            .iter()
            .collect(),
        &empty,
        &available_packages,
    );
    results.sort();
    results.iter().for_each(|package| {
        println!(
            "{} {}",
            Color::Purple.paint(package.name()),
            &package.version
        );
    });
}

pub fn install_packages(
    root_directory_path: &Path,
    package_names: BTreeSet<&str>,
    no_prompt: bool,
) {
    installer::check_for_pending_installation(root_directory_path, no_prompt);
    installer::install(
        root_directory_path,
        get_packages(root_directory_path, package_names, no_prompt),
    );
}

pub fn update_packages(root_directory_path: &Path, package_names: BTreeSet<&str>, no_prompt: bool) {
    installer::update(
        root_directory_path,
        get_packages(root_directory_path, package_names, no_prompt),
    )
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
        match utils::yes_or_no("Directory doesn't exist. Create it?", YES, false, None) {
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

fn get_packages(
    root_directory_path: &Path,
    package_names: BTreeSet<&str>,
    no_prompt: bool,
) -> BTreeSet<Package> {
    let path = paths::get_available_packages_file_path(root_directory_path);
    let available_packages = available_packages::get_packages(&path);
    let mut not_found: Vec<&str> = Vec::new();
    let mut packages: BTreeSet<Package> = BTreeSet::new();
    for name in package_names {
        match available_packages::latest_version(name, &available_packages) {
            Some(package) => {
                packages.insert(package.clone());
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
            if packages.is_empty()
                || utils::yes_or_no("Abort installation?", NO, no_prompt, None) == YES
            {
                process::exit(1);
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
            if packages.len() <= n || utils::yes_or_no("Abort?", NO, no_prompt, None) == YES {
                process::exit(1);
            }
        }
    }
    packages
}
