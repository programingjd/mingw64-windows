use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};

use ansi_term::Color;

use crate::commands;
use crate::commands::available_packages;
use crate::commands::installed_packages;
use crate::commands::packages::Package;
use crate::commands::paths;
use crate::commands::utils::yes_or_no;
use crate::commands::utils::YesNoAnswer::YES;

pub struct Cache {
    available_packages_file_path: PathBuf,
    installed_packages_file_path: PathBuf,
    pending_installations_file_path: PathBuf,
    installed_packages_backup_file_path: PathBuf,
    pub installed_packages: BTreeSet<Package>,
    pub available_packages: BTreeSet<Package>,
}

impl Cache {
    pub fn get(root_directory: &Path) -> Self {
        let available_packages_file_path = paths::get_available_packages_file_path(root_directory);
        let installed_packages_file_path = paths::get_installed_packages_file_path(root_directory);
        let pending_installations_file_path =
            paths::get_pending_installations_file_path(root_directory);
        let installed_packages_backup_file_path =
            paths::get_installed_packages_backup_file_path(root_directory);
        if pending_installations_file_path.exists() {
            if let Ok(s) = std::fs::read_to_string(&pending_installations_file_path) {
                if !s.is_empty() {
                    match Package::try_from(s.as_str()) {
                        Ok(ref package) => {
                            println!(
                                "Installation of the package {} did not finish successfully.",
                                Color::Purple.paint(&package.name)
                            );
                            let answer = yes_or_no("Retry?", YES);
                            let _ = rm_rf::remove(&pending_installations_file_path);
                            match answer {
                                YES => commands::install_package(root_directory, package),
                                _ => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        let available_packages = available_packages::get_packages(&available_packages_file_path);
        let installed_packages = installed_packages::get_packages(&installed_packages_file_path);
        Self {
            available_packages_file_path,
            installed_packages_file_path,
            pending_installations_file_path,
            installed_packages_backup_file_path,
            installed_packages,
            available_packages,
        }
    }
}
