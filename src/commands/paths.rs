use std::path::{Path, PathBuf};

pub fn get_installed_packages_file_path(root_directory: &Path) -> PathBuf {
    get_directory(root_directory).join("installed")
}

pub fn get_available_packages_file_path(root_directory: &Path) -> PathBuf {
    get_directory(root_directory).join("installed")
}

pub fn get_pending_installations_file_path(root_directory: &Path) -> PathBuf {
    get_directory(root_directory).join("installed")
}

pub fn get_installed_packages_backup_file_path(root_directory: &Path) -> PathBuf {
    get_directory(root_directory).join("backup")
}

fn get_directory(root_directory: &Path) -> PathBuf {
    root_directory.join("var").join("local").join("packages")
}
