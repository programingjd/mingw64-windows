extern crate lazy_static;

use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::io::Read;
use std::path::Path;

use crate::commands::errors::Result;
use crate::commands::packages::{Package, Packages};
use crate::commands::utils;
use crate::commands::utils::{Compression, ETag};

#[derive(Debug, Copy, Clone)]
pub enum Repository {
    Msys,
    Mingw64,
}

impl Display for Repository {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.name())
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct RepositoryVersion {
    pub etag: ETag,
    pub repository: &'static Repository,
}

lazy_static! {
    static ref ALL: Vec<&'static Repository> = vec![&Repository::Msys, &Repository::Mingw64];
}

impl PartialEq for Repository {
    fn eq(&self, other: &Self) -> bool {
        self.name().eq(other.name())
    }
    fn ne(&self, other: &Self) -> bool {
        self.name().ne(other.name())
    }
}

impl Eq for Repository {}

impl PartialOrd for Repository {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.name().partial_cmp(other.name())
    }
}

impl Ord for Repository {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name().cmp(other.name())
    }
}

impl Repository {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Msys => "msys",
            Self::Mingw64 => "mingw64",
        }
    }
    pub fn package_prefix(&self) -> &'static str {
        match self {
            Self::Msys => "",
            Self::Mingw64 => "mingw-w64-x86_64-",
        }
    }
    pub fn url(&self) -> &'static str {
        match self {
            Self::Msys => "https://repo.msys2.org/msys/x86_64/",
            Self::Mingw64 => "https://repo.msys2.org/mingw/x86_64/",
        }
    }
    pub fn enabled() -> &'static [&'static Self] {
        &ALL
    }
    pub fn from(name: &str) -> Option<&'static Self> {
        Self::enabled()
            .iter()
            .find(|&it| it.name() == name)
            .map(|it| *it)
    }
    fn db_url(&self) -> String {
        format!("{}{}.db", self.url(), self.name())
    }
    pub fn remote_etag(&self) -> Result<ETag> {
        utils::etag(&self.db_url())
    }
    /// Downloads the {repo}.db file that is in fact a tar.gz.
    /// The tar has one folder per package and inside each folder there's a desc file
    /// with package information.
    pub fn remote_packages(&'static self) -> Result<Packages> {
        let resp = utils::download(&self.db_url())?;
        let data = Compression::GZ.decompress(&resp.body)?;
        let mut tar = tar::Archive::new(data.as_slice());
        let entries = &mut tar.entries()?;
        Ok(Packages::create(
            RepositoryVersion {
                etag: resp.etag,
                repository: &self,
            },
            entries
                .filter_map(|it| it.ok())
                .filter(|entry| {
                    let path = String::from_utf8_lossy(&entry.path_bytes()).to_string();
                    path.ends_with("/desc")
                })
                .filter_map(|mut entry| {
                    let mut buf = Vec::with_capacity(entry.size() as usize);
                    entry
                        .read_to_end(&mut buf)
                        .ok()
                        .and_then(|_| String::from_utf8(buf).ok())
                        .and_then(|desc| self.read_description(&desc))
                })
                .collect(),
        ))
    }
    /// The desc file has sections separated by blank lines.
    /// Each section starts with a line containing the section name (%FILENAME%, %NAME%, ...)
    /// and then one line per value.
    fn read_description(&self, desc: &str) -> Option<Package> {
        // split into sections, then each section into lines
        let sections: Vec<Vec<_>> = desc
            .split("\n\n")
            .map(|section| section.split('\n').collect())
            .collect();
        // name is the unique value of the %NAME% section
        let names = Self::section_values("%NAME%", &sections)?;
        let name = names.first()?;
        // version is the unique value of the %VERSION% section
        let versions = Self::section_values("%VERSION%", &sections)?;
        let version = versions.first()?;
        // filename is the unique value of the %FILENAME% section
        // we deduce the package url and compression from it
        let filenames = Self::section_values("%FILENAME%", &sections)?;
        let filename = Path::new(filenames.first()?);
        // dependencies are the values of the %DEPENDS% section
        let dependencies = Self::section_values("%DEPENDS%", &sections).or(Some(vec![]));
        Some(Package {
            repository: Repository::from(&self.name())?,
            name: name.to_string(),
            version: version.to_string(),
            compression: Some(
                filename.extension().and_then(|ext| {
                    Compression::from_extension(&ext.to_string_lossy().to_string())
                })?,
            ),
            url: Some(format!(
                "{}{}",
                &self.url(),
                &filename.to_string_lossy().to_string()
            )),
            dependencies,
        })
    }
    /// Searches for the section with the specified section name and returns the value lines
    /// if it is found.
    fn section_values(key: &str, sections: &Vec<Vec<&str>>) -> Option<Vec<String>> {
        let section = sections.iter().find(|&it| {
            if let Some(&first) = it.first() {
                first == key
            } else {
                false
            }
        })?;
        Some(section.iter().skip(1).map(|&it| it.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let packages = Repository::Msys.remote_packages().unwrap();
        println!("ETag: {:?}", &packages.version.etag);
        packages
            .list
            .iter()
            .for_each(|package| println!("{}", &String::from(package)));
    }
}
