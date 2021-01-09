use crate::commands::packages::{Package, Packages};
use std::collections::BTreeSet;
use std::path::Path;

pub fn get_packages(installed_packages_file: &Path) -> BTreeSet<Package> {
    match Packages::get_packages_from_file(installed_packages_file) {
        Ok(packages) => packages,
        Err(_) => BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use std::path::PathBuf;

    lazy_static! {
        static ref DATA_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    }

    #[test]
    fn read_packages_from_file() {
        read_packages_from_file_at("installed_packages_file.zst")
    }

    fn read_packages_from_file_at(filename: &str) {
        let packages = Packages::get_packages_from_file(&DATA_DIR.join(filename)).unwrap();
        assert_eq!(packages.len(), 3);
        assert!(packages
            .get(&Package::try_from("msys package version").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("msys p1 1.0").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("mingw64 p2 3").unwrap())
            .is_some());
    }
}
