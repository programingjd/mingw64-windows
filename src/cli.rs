use crate::commands::root_directory;
use clap::{App, AppSettings, Arg};
use std::collections::BTreeSet;

mod commands;

#[macro_use]
extern crate lazy_static;

const LIST_INSTALLED_PACKAGES_COMMAND: &str = "installed";
const SEARCH_AVAILABLE_PACKAGES_COMMAND: &str = "search";
const INSTALL_PACKAGES_COMMAND: &str = "install";
const UPDATE_PACKAGES_COMMAND: &str = "update";
const LIST_DEPENDENCIES_COMMAND: &str = "dependencies";

fn main() {
    let _ = ansi_term::enable_ansi_support();
    let app = App::new("pwm")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Msys/Mingw64 packages installer")
        .arg(
            Arg::new("no-prompt")
                .short('y')
                .long("no-prompt")
                .about("Disable confirmation prompts."),
        )
        .subcommand(
            App::new(LIST_INSTALLED_PACKAGES_COMMAND)
                .about("list installed packages")
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
            App::new(LIST_DEPENDENCIES_COMMAND)
                .about("list dependencies of the specified packages")
                .arg(
                    Arg::new("name")
                        .about("The name of the packages to update")
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
        .subcommand(
            App::new(UPDATE_PACKAGES_COMMAND)
                .about("update packages")
                .arg(
                    Arg::new("name")
                        .about("The name of the packages to update")
                        .required(false)
                        .multiple(true),
                ),
        )
        .setting(AppSettings::ArgRequiredElseHelp);
    let matches = app.get_matches();
    let no_prompt = matches.occurrences_of("no-prompt") > 0;
    if let Some(matches) = matches.subcommand_matches(LIST_INSTALLED_PACKAGES_COMMAND) {
        if let Some(packages) = matches.values_of("package") {
            let packages: BTreeSet<_> = packages.collect();
            commands::list_installed_packages(&root_directory(no_prompt), packages);
        } else {
            let root_directory = commands::root_directory(no_prompt);
            commands::list_all_installed_packages(&root_directory);
        }
    } else if let Some(matches) = matches.subcommand_matches(SEARCH_AVAILABLE_PACKAGES_COMMAND) {
        if let Some(terms) = matches.values_of("term") {
            let terms: BTreeSet<_> = terms.collect();
            commands::search_available_packages(&root_directory(no_prompt), terms);
        }
    } else if let Some(matches) = matches.subcommand_matches(LIST_DEPENDENCIES_COMMAND) {
        if let Some(names) = matches.values_of("name") {
            let names: BTreeSet<_> = names.collect();
            commands::list_dependencies(&root_directory(no_prompt), names, no_prompt);
        }
    } else if let Some(matches) = matches.subcommand_matches(INSTALL_PACKAGES_COMMAND) {
        if let Some(names) = matches.values_of("name") {
            let names: BTreeSet<_> = names.collect();
            commands::install_packages(&root_directory(no_prompt), names, no_prompt);
        }
    } else if let Some(matches) = matches.subcommand_matches(UPDATE_PACKAGES_COMMAND) {
        if let Some(names) = matches.values_of("name") {
            let names: BTreeSet<_> = names.collect();
            commands::update_packages(&root_directory(no_prompt), names, no_prompt);
        } else {
            commands::update_packages(&root_directory(no_prompt), BTreeSet::new(), no_prompt);
        }
    }
}
