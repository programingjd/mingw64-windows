use std::path::{Path, PathBuf};

use crate::commands::packages::Package;
use crate::commands::utils::YesNoAnswer::{NO, YES};
use ansi_term::Color;
use std::env;
use std::fs;
use std::process::exit;

mod available_packages;
mod cache;
mod errors;
mod installed_packages;
mod packages;
mod paths;
mod repositories;
mod utils;

/// Automatically selects the current directory if ./var/lib/packages/installed exists, otherwise
/// asks the user.
pub fn root_directory() -> PathBuf {
    if let Ok(current_directory) = env::current_dir() {
        if paths::get_installed_packages_file_path(&current_directory).exists() {
            current_directory
        } else {
            prompt_for_directory(Some(&current_directory))
        }
    } else {
        prompt_for_directory(None)
    }
}

pub fn list_installed_packages(root_directory: &Path) {
    if paths::get_installed_packages_file_path(root_directory).exists() {
        let path = paths::get_installed_packages_file_path(root_directory);
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

pub fn install_package(root_directory: &Path, package: &Package) {
    todo!()
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
                Err(err) => {
                    println!("{}", Color::Red.paint("Failed to create the directory."));
                    exit(1)
                }
            },
            NO => exit(0),
        }
    }
}
