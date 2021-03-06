use crate::commands::errors::Result;
use crate::commands::packages::{Package, Packages};
use crate::commands::paths;
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn get_packages(installed_packages_file_path: &Path) -> BTreeSet<Package> {
    match Packages::get_packages_from_file(installed_packages_file_path) {
        Ok(packages) => packages,
        Err(_) => BTreeSet::new(),
    }
}

pub fn append_package(root_directory_path: &Path, package: &Package) -> Result<()> {
    let installed_packages_file_path = paths::get_installed_packages_file_path(root_directory_path);
    let mut bytes = if let Ok(metadata) = fs::metadata(&installed_packages_file_path) {
        if metadata.len() > 0 {
            zstd::decode_all(File::open(&installed_packages_file_path)?)?
        } else {
            // file is empty
            vec![]
        }
    } else {
        // file doesn't exist
        File::create(&installed_packages_file_path)?;
        vec![]
    };
    bytes.append("\n".as_bytes().to_vec().as_mut());
    bytes.append(String::from(package).as_bytes().to_vec().as_mut());
    let bytes = zstd::encode_all(bytes.as_slice(), zstd::DEFAULT_COMPRESSION_LEVEL)?;
    fs::write(&installed_packages_file_path, &bytes)?;
    // make a copy of the installed packages file as a backup
    let backup_file_path = paths::get_installed_packages_backup_file_path(root_directory_path);
    let _ = fs::write(&backup_file_path, &bytes);
    Ok(())
}

pub fn replace_package(root_directory_path: &Path, package: &Package) -> Result<()> {
    let installed_packages_file_path = paths::get_installed_packages_file_path(root_directory_path);
    let input: Vec<_> = File::open(&installed_packages_file_path)
        .map(|file| BufReader::new(file).lines())?
        .collect();
    let mut output = Vec::new();
    for line in input {
        let line = line?;
        let current = Package::try_from(line.as_str())?;
        if current.matches(package.name()) {
            output.push(String::from(&current));
        } else {
            output.push(line);
        }
    }
    let output = output.join("\n");
    let bytes = output.as_bytes();
    fs::write(&installed_packages_file_path, bytes)?;
    // make a copy of the installed packages file as a backup
    let backup_file_path = paths::get_installed_packages_backup_file_path(root_directory_path);
    let _ = fs::write(&backup_file_path, bytes);
    Ok(())
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
            .get(&Package::try_from("msys\tpackage, p\tversion").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("msys\tp1\t1.0").unwrap())
            .is_some());
        assert!(packages
            .get(&Package::try_from("mingw64\tp2\t3").unwrap())
            .is_some());
    }
}
