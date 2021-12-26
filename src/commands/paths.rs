use crate::commands::utils;
use crate::commands::utils::YesNoAnswer::YES;
use ansi_term::Color;
use std::path::{Path, PathBuf};
use std::process;
use std::{env, fs};

pub fn create_directory_structure(root_directory_path: &Path, no_prompt: bool) {
    if !root_directory_path.exists() {
        match utils::yes_or_no("Directory doesn't exist. Create it?", YES, no_prompt, None) {
            YES => {
                if let Err(_) = fs::create_dir_all(root_directory_path) {
                    println!("{}", Color::Red.paint("Failed to create the directory."));
                    process::exit(1)
                }
            }
            _ => process::exit(0),
        }
    }
    // create var/local/packages
    create_dir_if_missing(&get_directory(root_directory_path));

    // create /usr
    let usr = root_directory_path.join("usr");
    create_dir_if_missing(&usr);
    // create /usr/bin
    let usr_bin = usr.join("bin");
    create_dir_if_missing(&usr_bin);
    // link /bin to /usr/bin
    create_junction_if_missing(&root_directory_path.join("bin"), &usr_bin);
    // create /usr/include
    let usr_include = usr.join("include");
    create_dir_if_missing(&usr_include);
    // link /include to /usr/include
    create_junction_if_missing(&root_directory_path.join("include"), &usr_include);
    // create /usr/lib
    let usr_lib = usr.join("lib");
    create_dir_if_missing(&usr_lib);
    // link /lib to /usr/lib
    create_junction_if_missing(&root_directory_path.join("lib"), &usr_lib);
    // create /usr/libexec
    let usr_libexec = usr.join("libexec");
    create_dir_if_missing(&usr_libexec);
    // link /libexec to /usr/libexec
    create_junction_if_missing(&root_directory_path.join("libexec"), &usr_libexec);
    // create /usr/share
    let usr_share = usr.join("share");
    create_dir_if_missing(&usr_share);
    // link /share to /usr/share
    create_junction_if_missing(&root_directory_path.join("share"), &usr_share);

    // create /etc
    let etc = root_directory_path.join("etc");
    create_dir_if_missing(&etc);
    // link /usr/etc to /etc
    create_junction_if_missing(&usr.join("etc"), &etc);
    // create /tmp
    let tmp = root_directory_path.join("tmp");
    create_dir_if_missing(&tmp);
    // link /usr/tmp to /tmp
    create_junction_if_missing(&usr.join("tmp"), &tmp);
    // link /usr/var to /var
    create_junction_if_missing(&usr.join("var"), &root_directory_path.join("var"));

    // create /home and user directory
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

    // link /usr/x86_64-pc-msys to /usr
    create_junction_if_missing(&usr.join("x86_64-pc-msys"), &usr);
    // link /usr/x86_64-w64-mingw32 to /usr
    create_junction_if_missing(&usr.join("x86_64-w64-mingw32"), &usr);
    // link /mingw64 to /usr
    create_junction_if_missing(&root_directory_path.join("mingw64"), &usr);
    // link /usr/local to /usr
    create_junction_if_missing(&usr.join("local"), &usr);
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
export PATH=/usr/bin
export PS1="\[\e[35m\]\u@\h\[\e[m\]:\[\e[33m\]\w\[\e[m\]\\$"
export TERM=cygwin
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
    get_directory(root_directory_path).join("available")
}

pub fn get_pending_installation_file_path(root_directory_path: &Path) -> PathBuf {
    get_directory(root_directory_path).join("pending")
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
