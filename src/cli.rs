use clap::{App, AppSettings};
mod commands;

#[macro_use]
extern crate lazy_static;

const LIST_INSTALLED_PACKAGES_COMMAND: &str = "installed";

fn main() {
    let app = App::new("pwm")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Msys/Mingw64 packages installer")
        .subcommand(App::new(LIST_INSTALLED_PACKAGES_COMMAND).about("lists installed packages"))
        .setting(AppSettings::ArgRequiredElseHelp);
    let matches = app.get_matches();
    if let Some(_) = matches.subcommand_matches(LIST_INSTALLED_PACKAGES_COMMAND) {
        let root_directory = commands::root_directory();
        commands::list_installed_packages(&root_directory);
    }
}
