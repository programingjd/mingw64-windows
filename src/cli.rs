use crate::commands::root_directory;
use clap::{App, AppSettings, Arg};
use std::collections::BTreeSet;

mod commands;

#[macro_use]
extern crate lazy_static;

const LIST_INSTALLED_PACKAGES_COMMAND: &str = "installed";
const SEARCH_AVAILABLE_PACKAGES_COMMAND: &str = "search";
const INSTALL_PACKAGES_COMMAND: &str = "install";

fn main() {
    let _ = ansi_term::enable_ansi_support();
    let app = App::new("pwm")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Msys/Mingw64 packages installer")
        .subcommand(
            App::new(LIST_INSTALLED_PACKAGES_COMMAND)
                .about("lists installed packages")
                .arg(
                    Arg::new("package")
                        .about("Looks for the specified package(s) only")
                        .required(false)
                        .multiple(true),
                ),
        )
        .subcommand(
            App::new(SEARCH_AVAILABLE_PACKAGES_COMMAND)
                .about("search available packages")
                .arg(
                    Arg::new("term")
                        .about("The term to look for in the package name")
                        .required(true)
                        .multiple(true),
                ),
        )
        .subcommand(
            App::new(INSTALL_PACKAGES_COMMAND)
                .about("install packages")
                .arg(
                    Arg::new("name")
                        .about("The name of the package to install")
                        .required(true)
                        .multiple(true),
                ),
        )
        .setting(AppSettings::ArgRequiredElseHelp);
    let matches = app.get_matches();
    if let Some(matches) = matches.subcommand_matches(LIST_INSTALLED_PACKAGES_COMMAND) {
        if let Some(packages) = matches.values_of("package") {
            let packages: BTreeSet<_> = packages.collect();
            commands::list_installed_packages(&root_directory(), packages);
        } else {
            let root_directory = commands::root_directory();
            commands::list_all_installed_packages(&root_directory);
        }
    } else if let Some(matches) = matches.subcommand_matches(SEARCH_AVAILABLE_PACKAGES_COMMAND) {
        if let Some(terms) = matches.values_of("term") {
            let terms: BTreeSet<_> = terms.collect();
            commands::search_available_packages(&root_directory(), terms);
        }
    } else if let Some(matches) = matches.subcommand_matches(INSTALL_PACKAGES_COMMAND) {
        if let Some(names) = matches.values_of("name") {
            let names: BTreeSet<_> = names.collect();
            commands::install_packages(&root_directory(), names);
        }
    }
}
