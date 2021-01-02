use std::path::{Path, PathBuf};

use crate::commands::packages::Package;

mod cache;
mod errors;
mod packages;
mod repositories;
mod utils;

/// Automatically selects the current directory if ./var/lib/packages/installed exists, otherwise
/// asks the user.
pub fn root_directory() -> PathBuf {
    todo!()
}

pub fn list_installed_packages(root_directory: &Path) {
    cache::Cache::get(root_directory);
}

pub fn install_package(root_directory: &Path, package: &Package) {
    todo!()
}
