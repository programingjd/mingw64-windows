use std::convert::TryFrom;
use std::result::Result as StdResult;

use crate::commands::errors::{Error, Error::ParseError};
use crate::commands::repositories::Repository;
use crate::commands::utils::Compression;

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Package {
    pub repository: &'static Repository,
    pub name: String,
    pub version: String,
    pub compression: &'static Compression,
    pub url: String,
    pub dependencies: Option<Vec<String>>,
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
                .skip(pos)
                .map(|it| it.to_string())
                .collect()
        });
        let url = format!(
            "{}{}{}{}",
            &repository.url(),
            &name,
            &version,
            &compression.extension()
        );
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
