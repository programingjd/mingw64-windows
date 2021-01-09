use crate::commands::root_directory;
use clap::{App, AppSettings, Arg};
use std::collections::BTreeSet;

mod commands;

#[macro_use]
extern crate lazy_static;

const LIST_INSTALLED_PACKAGES_COMMAND: &str = "installed";

fn main() {
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
        .setting(AppSettings::ArgRequiredElseHelp);
    let matches = app.get_matches();
    if let Some(matches) = matches.subcommand_matches(LIST_INSTALLED_PACKAGES_COMMAND) {
        if let Some(packages) = matches.values_of("package") {
            let packages: BTreeSet<_> = packages.collect();
            commands::list_installed_packages(&root_directory(), packages)
        } else {
            let root_directory = commands::root_directory();
            commands::list_all_installed_packages(&root_directory);
        }
    }
}
