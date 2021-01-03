use std::convert::TryFrom;
use std::result::Result as StdResult;

use crate::commands::errors::{Error, Error::ParseError};
use crate::commands::repositories::{Repository, RepositoryVersion};
use crate::commands::utils::Compression;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct Package {
    pub repository: &'static Repository,
    pub name: String,
    pub version: String,
    pub compression: &'static Compression,
    pub url: String,
    pub dependencies: Option<Vec<String>>,
}

impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version
    }
    fn ne(&self, other: &Self) -> bool {
        self.name != other.name || self.version != other.version
    }
}

impl Eq for Package {}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let cmp = self.name.partial_cmp(&other.name);
        if let Some(Ordering::Equal) = cmp {
            self.version.partial_cmp(&other.version)
        } else {
            cmp
        }
    }
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = self.name.cmp(&other.name);
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
        let cols: Vec<&str> = value.split(' ').collect();
        if cols.len() < 4 {
            return Err(ParseError);
        }
        let repository = Repository::from(cols[0]).ok_or(ParseError)?;
        let name = cols[1].to_string();
        let version = cols[2].to_string();
        let compression = Compression::from_extension(cols[3]).ok_or(ParseError)?;
        let dependencies = cols.iter().position(|&it| it == "+").map(|pos| {
            cols.into_iter()
                .skip(pos + 1)
                .map(|it| it.to_string())
                .collect()
        });
        let url = url_from(repository, &name, &version, compression);
        Ok(Package {
            repository,
            name,
            version,
            compression,
            url,
            dependencies,
        })
    }
}

fn url_from(
    repository: &Repository,
    name: &str,
    version: &str,
    compression: &Compression,
) -> String {
    format!(
        "{}{}{}{}",
        &repository.url(),
        &name,
        &version,
        &compression.extension()
    )
}

/// {repo_name} {package_name} {package_version} {compression_extension}
/// followed by optional dependencies: + {package_name_with_optional_version_constraints} ...
impl From<&Package> for String {
    fn from(package: &Package) -> Self {
        let mut cols = vec![
            package.repository.name(),
            &package.name,
            &package.version,
            package.compression.extension(),
        ];
        if let Some(ref deps) = package.dependencies {
            cols.push("+");
            deps.iter().for_each(|dep| cols.push(dep));
        }
        cols.join(" ")
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_with_dependencies() {
        let package =
            Package::try_from("msys name version zst + dep1 dep2=1 dep3>3.2 dep4").unwrap();
        assert_eq!(package.repository, &Repository::Msys);
        assert_eq!(package.name, "name");
        assert_eq!(package.version, "version");
        assert_eq!(package.compression, &Compression::ZSTD);
        let dependencies = package.dependencies.unwrap();
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
        assert!(package.url.starts_with(package.repository.url()));
        assert!(package.url.ends_with(package.compression.extension()));
    }

    #[test]
    fn test_parsing_with_empty_dependencies() {
        let package = Package::try_from("mingw64 package 1.0 xz +").unwrap();
        assert_eq!(package.repository, &Repository::Mingw64);
        assert_eq!(package.name, "package");
        assert_eq!(package.version, "1.0");
        assert_eq!(package.compression, &Compression::XZ);
        let dependencies = package.dependencies.unwrap();
        assert!(dependencies.is_empty());
        assert!(package.url.starts_with(package.repository.url()));
        assert!(package.url.ends_with(package.compression.extension()));
    }

    #[test]
    fn test_parsing_without_dependencies() {
        let package = Package::try_from("msys name version zst").unwrap();
        assert_eq!(package.repository, &Repository::Msys);
        assert_eq!(package.name, "name");
        assert_eq!(package.version, "version");
        assert_eq!(package.compression, &Compression::ZSTD);
        assert!(package.dependencies.is_none());
        assert!(package.url.starts_with(package.repository.url()));
        assert!(package.url.ends_with(package.compression.extension()));
    }

    #[test]
    fn test_formatting_with_dependencies() {
        let repository = &Repository::Msys;
        let name = "name".to_string();
        let version = "version".to_string();
        let compression = &Compression::ZSTD;
        let url = url_from(repository, &name, &version, compression);
        let dep1 = "dep1".to_string();
        let dep2 = "dep2=1.0".to_string();
        let dep3 = "dep3".to_string();
        let dep4 = "dep4>0".to_string();
        let dep5 = "dep5".to_string();
        let package = Package {
            repository,
            name,
            version,
            compression,
            url,
            dependencies: Some(vec![dep1, dep2, dep3, dep4, dep5]),
        };
        assert_eq!(
            &String::from(&package),
            "msys name version zst + dep1 dep2=1.0 dep3 dep4>0 dep5"
        )
    }

    #[test]
    fn test_formatting_without_dependencies() {
        let repository = &Repository::Mingw64;
        let name = "package".to_string();
        let version = "1.0".to_string();
        let compression = &Compression::XZ;
        let url = url_from(repository, &name, &version, compression);
        let package = Package {
            repository,
            name,
            version,
            compression,
            url,
            dependencies: None,
        };
        assert_eq!(&String::from(&package), "mingw64 package 1.0 xz")
    }

    #[test]
    fn test_cant_parse() {
        assert!(Package::try_from("").is_err());
        assert!(Package::try_from("msys").is_err());
        assert!(Package::try_from("msys name").is_err());
        assert!(Package::try_from("msys name version").is_err());
        assert!(Package::try_from("unknown_repo name 1.0 zst").is_err());
        assert!(Package::try_from("msys name 1.0 unknown_ext").is_err());
    }

    #[test]
    fn test_eq() {
        let package = Package::try_from("msys a 1 zst + d1 d2").unwrap();
        assert_eq!(package, Package::try_from("msys a 1 zst + d1 d2").unwrap());
        assert_eq!(
            package,
            Package::try_from("mingw64 a 1 zst + d1 d2").unwrap()
        );
        assert_eq!(package, Package::try_from("msys a 1 zst +").unwrap());
        assert_eq!(package, Package::try_from("msys a 1 zst").unwrap());
        assert_eq!(package, Package::try_from("msys a 1 xz").unwrap());
        assert_ne!(package, Package::try_from("msys b 1 zst + d1 d2").unwrap());
        assert_ne!(package, Package::try_from("msys a 2 zst + d1 d2").unwrap());
    }
}
