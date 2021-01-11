use crate::commands::utils;
use crate::commands::utils::YesNoAnswer::YES;
use ansi_term::Color;
use std::path::{Path, PathBuf};
use std::process;
use std::{env, fs};

pub fn create_directory_structure(root_directory_path: &Path) {
    if !root_directory_path.exists() {
        match utils::yes_or_no("Directory doesn't exist. Create it?", YES) {
            YES => {
                if let Err(_) = fs::create_dir_all(root_directory_path) {
                    println!("{}", Color::Red.paint("Failed to create the directory."));
                    process::exit(1)
                }
            }
            _ => process::exit(0),
        }
    }
    let path = get_directory(root_directory_path);
    create_dir_if_missing(&path);
    let path = root_directory_path.join("bin");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("etc");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("home");
    create_dir_if_missing(&path);
    if let Ok(user) = env::var("USERNAME") {
        if let Ok(target) = env::var("USERPROFILE") {
            let path = path.join(&user);
            let target = Path::new(&target);
            create_junction_if_missing(&path, target);
            create_bashrc_if_missing(&path);
        }
    }
    let path = root_directory_path.join("include");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("lib");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("libexec");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("mingw64");
    create_junction_if_missing(&path, root_directory_path);
    let path = root_directory_path.join("share");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("tmp");
    create_dir_if_missing(&path);
    let path = root_directory_path.join("usr");
    create_junction_if_missing(&path, root_directory_path);
}

fn create_dir_if_missing(path: &Path) {
    if !path.exists() {
        if let Err(_) = fs::create_dir_all(path) {
            println!(
                "{}",
                Color::Red.paint("Failed to create the directory structure.")
            );
            process::exit(1)
        }
    }
}

fn create_junction_if_missing(path: &Path, target: &Path) {
    if !junction::exists(&path).unwrap_or(false) {
        if let Err(_) = junction::create(target, &path) {
            println!(
                "{}",
                Color::Red.paint("Failed to create the directory structure.")
            );
            process::exit(1)
        }
    }
}

fn create_bashrc_if_missing(home_directory: &Path) {
    let file = home_directory.join(".bashrc");
    if !file.exists() {
        let _ = fs::write(
            &file,
            r###"
export PATH=/bin
export PS1="\[\e[35m\]\u@\h\[\e[m\]:\[\e[33m\]\w\[\e[m\]\\$"
HISCONTROL=ignoreboth
shopt -s histappend
HISTSIZE=1000
HISTFILESIZE=16000

alias ll="ls -la"
alias cd..="cd .."
            "###,
        );
    }
}

pub fn get_installed_packages_file_path(root_directory_path: &Path) -> PathBuf {
    get_directory(root_directory_path).join("installed")
}

pub fn get_available_packages_file_path(root_directory_path: &Path) -> PathBuf {
    get_directory(root_directory_path).join("installed")
}

pub fn get_pending_installation_file_path(root_directory_path: &Path) -> PathBuf {
    get_directory(root_directory_path).join("installed")
}

pub fn get_installed_packages_backup_file_path(root_directory_path: &Path) -> PathBuf {
    get_directory(root_directory_path).join("backup")
}

fn get_directory(root_directory_path: &Path) -> PathBuf {
    root_directory_path
        .join("var")
        .join("local")
        .join("packages")
}
