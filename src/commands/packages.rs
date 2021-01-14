use std::convert::TryFrom;
use std::result::Result as StdResult;

use crate::commands::errors::Error::IOError;
use crate::commands::errors::{Error, Error::ParseError, Result};
use crate::commands::repositories::{Repository, RepositoryVersion};
use crate::commands::utils::Compression;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

const PACKAGE_EXTENSION: &'static str = "pkg.tar";

#[derive(Debug, Clone)]
pub struct Package {
    pub repository: &'static Repository,
    pub names: Vec<String>,
    pub version: String,
    pub compression: Option<&'static Compression>,
    pub arch: Option<String>,
    pub dependencies: Option<Vec<String>>,
}

impl Package {
    pub fn name(&self) -> &str {
        self.names.first().unwrap()
    }
    pub fn matches(&self, name: &str) -> bool {
        self.names.iter().any(|it| it == name)
    }
    pub fn url(&self) -> Option<String> {
        if let Some(compression) = self.compression {
            if let Some(ref arch) = self.arch {
                Some(url_from(
                    self.repository,
                    self.name(),
                    self.version.as_str(),
                    compression,
                    arch.as_str(),
                ))
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name() && self.version == other.version
    }
    fn ne(&self, other: &Self) -> bool {
        self.name() != other.name() || self.version != other.version
    }
}

impl Eq for Package {}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let cmp = self.name().partial_cmp(other.name());
        if let Some(Ordering::Equal) = cmp {
            self.version.partial_cmp(&other.version)
        } else {
            cmp
        }
    }
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = self.name().cmp(other.name());
        if Ordering::Equal == cmp {
            self.version.cmp(&other.version)
        } else {
            cmp
        }
    }
}

/// {repo_name} {package_name} {package_version} {compression_extension}
/// followed by optional dependencies: + {package_name_with_optional_version_constraints} ...
impl TryFrom<&str> for Package {
    type Error = Error;
    fn try_from(value: &str) -> StdResult<Self, Self::Error> {
        let cols: Vec<&str> = value.split('\t').collect();
        if cols.len() < 3 {
            return Err(ParseError);
        }
        let repository = Repository::from(cols[0]).ok_or(ParseError)?;
        let names = cols[1]
            .to_string()
            .split(", ")
            .map(|it| it.to_string())
            .collect();
        let version = cols[2].to_string();
        let compression = if let Some(&col) = cols.get(3) {
            Some(Compression::from_extension(col).ok_or(ParseError)?)
        } else {
            None
        };
        let arch = if let Some(&col) = cols.get(4) {
            Some(col.to_string())
        } else {
            None
        };
        let dependencies = cols.iter().position(|&it| it == "+").map(|pos| {
            cols.into_iter()
                .skip(pos + 1)
                .map(|it| it.to_string())
                .collect()
        });
        Ok(Package {
            repository,
            names,
            version,
            compression,
            arch,
            dependencies,
        })
    }
}

fn url_from(
    repository: &Repository,
    name: &str,
    version: &str,
    compression: &Compression,
    arch: &str,
) -> String {
    format!(
        "{}{}-{}-{}.{}.{}",
        &repository.url(),
        name,
        version,
        arch,
        PACKAGE_EXTENSION,
        &compression.extension()
    )
}

/// {repo_name} {package_name} {package_version} {compression_extension}
/// followed by optional dependencies: + {package_name_with_optional_version_constraints} ...
impl From<&Package> for String {
    fn from(package: &Package) -> Self {
        let names = package.names.join(", ");
        let mut cols = vec![package.repository.name(), names.as_str(), &package.version];
        if let Some(ref compression) = package.compression {
            cols.push(compression.extension())
        }
        if let Some(ref arch) = package.arch {
            cols.push(arch);
        }
        if let Some(ref deps) = package.dependencies {
            cols.push("+");
            deps.iter().for_each(|dep| cols.push(dep));
        }
        cols.join("\t")
    }
}

#[derive(Debug)]
pub struct Packages {
    pub version: RepositoryVersion,
    pub list: Vec<Package>,
}

impl Packages {
    pub fn create(version: RepositoryVersion, packages: Vec<Package>) -> Self {
        Self {
            version,
            list: packages,
        }
    }
    pub fn get_packages_from_file(available_packages_file: &Path) -> Result<BTreeSet<Package>> {
        let decoder = zstd::Decoder::new(File::open(available_packages_file)?)?;
        BufReader::new(decoder)
            .lines()
            .skip(1)
            .map(|it| {
                it.map_err(|err| IOError(err))
                    .and_then(|line| Package::try_from(line.as_str()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    lazy_static! {
        static ref DATA_DIR: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    }

    #[test]
    fn test_parsing_with_dependencies() {
        let package =
            Package::try_from("msys\tname\tversion\tzst\t+\tdep1\tdep2=1\tdep3>3.2\tdep4").unwrap();
        assert_eq!(package.repository, &Repository::Msys);
        assert_eq!(package.name(), "name");
        assert_eq!(1, package.names.len());
        assert_eq!(package.version, "version");
        assert!(package.compression.is_some());
        assert_eq!(package.compression.unwrap(), &Compression::ZSTD);
        let dependencies = package.dependencies.as_ref().unwrap();
        assert_eq!(
            dependencies.len(),
            4,
            "Expected 4 dependencies but got: {}: {:?}",
            dependencies.len(),
            dependencies
        );
        assert_eq!(dependencies.get(0).unwrap(), "dep1");
        assert_eq!(dependencies.get(1).unwrap(), "dep2=1");
        assert_eq!(dependencies.get(2).unwrap(), "dep3>3.2");
        assert_eq!(dependencies.get(3).unwrap(), "dep4");
        assert!(package.url().is_some());
        let url = package.url();
        let url = url.as_ref().unwrap();
        assert!(url.starts_with(package.repository.url()));
        assert!(url.ends_with(&format!(
            ".{}.{}",
            PACKAGE_EXTENSION,
            package.compression.unwrap().extension()
        )));
    }

    #[test]
    fn test_parsing_with_empty_dependencies() {
        let package = Package::try_from("mingw64\tpackage, package-git\t1.0\txz\t+").unwrap();
        assert_eq!(package.repository, &Repository::Mingw64);
        assert_eq!(package.name(), "package");
        assert_eq!(2, package.names.len());
        assert_eq!(package.version, "1.0");
        assert!(package.compression.is_some());
        assert_eq!(package.compression.unwrap(), &Compression::XZ);
        let dependencies = package.dependencies.as_ref().unwrap();
        assert!(dependencies.is_empty());
        assert!(package.url().is_some());
        let url = package.url().unwrap();
        assert!(url.starts_with(package.repository.url()));
        assert!(url.ends_with(&format!(
            ".{}.{}",
            PACKAGE_EXTENSION,
            package.compression.unwrap().extension()
        )));
    }

    #[test]
    fn test_parsing_without_dependencies() {
        let package = Package::try_from("msys\tname\tversion\tzst\tany").unwrap();
        assert_eq!(package.repository, &Repository::Msys);
        assert_eq!(package.name(), "name");
        assert_eq!(1, package.names.len());
        assert_eq!(package.version, "version");
        assert!(package.compression.is_some());
        assert_eq!(package.compression.unwrap(), &Compression::ZSTD);
        assert!(package.dependencies.is_none());
        assert!(package.url().is_some());
        let url = package.url().unwrap();
        assert!(url.starts_with(package.repository.url()));
        assert!(url.ends_with(&format!(
            ".{}.{}",
            PACKAGE_EXTENSION,
            package.compression.unwrap().extension()
        )));
    }

    #[test]
    fn test_parsing_without_compression() {
        let package = Package::try_from("msys\tname\tversion").unwrap();
        assert_eq!(package.repository, &Repository::Msys);
        assert_eq!(package.name(), "name");
        assert_eq!(package.version, "version");
        assert!(package.compression.is_none());
        assert!(package.dependencies.is_none());
        assert!(package.url().is_none());
    }

    #[test]
    fn test_formatting_with_dependencies() {
        let repository = &Repository::Msys;
        let name = "name".to_string();
        let version = "version".to_string();
        let compression = &Compression::ZSTD;
        let arch = "any";
        let compression = Some(compression);
        let dep1 = "dep1".to_string();
        let dep2 = "dep2=1.0".to_string();
        let dep3 = "dep3".to_string();
        let dep4 = "dep4>0".to_string();
        let dep5 = "dep5".to_string();
        let package = Package {
            repository,
            names: vec![name],
            version,
            compression,
            arch: Some(arch.to_string()),
            dependencies: Some(vec![dep1, dep2, dep3, dep4, dep5]),
        };
        assert_eq!(
            &String::from(&package),
            "msys\tname\tversion\tzst\tany\t+\tdep1\tdep2=1.0\tdep3\tdep4>0\tdep5"
        )
    }

    #[test]
    fn test_formatting_without_dependencies() {
        let repository = &Repository::Mingw64;
        let name1 = "package".to_string();
        let name2 = "package-git".to_string();
        let version = "1.0".to_string();
        let compression = &Compression::XZ;
        let arch = "x86_64";
        let compression = Some(compression);
        let package = Package {
            repository,
            names: vec![name1, name2],
            version,
            compression,
            arch: Some(arch.to_string()),
            dependencies: None,
        };
        assert_eq!(
            &String::from(&package),
            "mingw64\tpackage, package-git\t1.0\txz\tx86_64"
        )
    }

    #[test]
    fn test_cant_parse() {
        assert!(Package::try_from("").is_err());
        assert!(Package::try_from("msys").is_err());
        assert!(Package::try_from("msys\tname").is_err());
        assert!(Package::try_from("msys\tname, name2").is_err());
        assert!(Package::try_from("unknown_repo\tname\t1.0\tzst\tx86_64").is_err());
        assert!(Package::try_from("msys\tname\t-\t1.0\tunknown_ext\tany").is_err());
    }

    #[test]
    fn test_eq() {
        let package = Package::try_from("msys\ta\t1\tzst\tany\t+\td1\td2").unwrap();
        assert_eq!(
            package,
            Package::try_from("mingw64\ta\t1\txz\tx86_64\t+\td1\td2\td3").unwrap()
        );
        assert_eq!(
            package,
            Package::try_from("msys\ta, b\t1\tzst\tx86_64").unwrap()
        );
        assert_eq!(
            package,
            Package::try_from("msys\ta\t1\tzst\tx86_64").unwrap()
        );
        assert_eq!(
            package,
            Package::try_from("msys\ta\t1\tzst\tany\t+").unwrap()
        );
        assert_eq!(
            package,
            Package::try_from("msys\ta\t1\tzst\tx86_64").unwrap()
        );
        assert_eq!(package, Package::try_from("msys\ta\t1\txz\tany").unwrap());
        assert_ne!(
            package,
            Package::try_from("msys\tb\t1\tzst\t+\td1\td2").unwrap()
        );
        assert_ne!(
            package,
            Package::try_from("msys\ta\t2\tzst\t+\td1\td2").unwrap()
        );
    }

    #[test]
    fn read_packages_from_non_existing_file() {
        assert!(Packages::get_packages_from_file(&DATA_DIR.join("non_existing_file.zst")).is_err());
    }
}
