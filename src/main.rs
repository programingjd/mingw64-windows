mod commands;

#[macro_use]
extern crate lazy_static;

use ansi_term::Color;
use regex::Regex;
use scraper::{Html, Selector};
use std::borrow::Borrow;
use std::collections::{BTreeSet, HashMap, LinkedList};
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::exit;

const MSYS_REPO_URL: &str = "https://repo.msys2.org/msys/x86_64/";
const MINGW64_REPO_URL: &str = "https://repo.msys2.org/mingw/x86_64/";
const MINGW64_PKG_PREFIX: &str = "mingw-w64-x86_64-";

enum Compression {
    ZSTD,
    XZ,
}

impl Display for Compression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ZSTD => "zst",
            Self::XZ => "xz",
        })
    }
}

struct Package {
    url: String,
    compression: Compression,
}

struct Requirements {
    set: BTreeSet<String>,
    list: LinkedList<String>,
}

impl Requirements {
    pub fn new() -> Self {
        Self {
            set: BTreeSet::new(),
            list: LinkedList::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
    pub fn add(&mut self, dependency: String) -> bool {
        if self.set.contains(dependency.as_str()) {
            false
        } else {
            self.list.push_front(dependency);
            self.set.insert(self.list.front().unwrap().to_string());
            true
        }
    }
    pub fn append(&mut self, dependency: String) {
        if !self.set.contains(dependency.as_str()) {
            self.list.push_back(dependency);
            self.set.insert(self.list.back().unwrap().to_string());
        }
    }
    pub fn take(&mut self) -> Option<String> {
        self.list.pop_back().and_then(|it| {
            self.set.remove(&it);
            Some(it)
        })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().find(|it| *it == "-h" || *it == "--help") {
        Some(_) => {
            let path = PathBuf::from(std::env::args().find(|_| true).unwrap());
            println!(
                "Usage: {} [OPTION]... [PACKAGE]...",
                path.file_stem().unwrap().to_string_lossy()
            );
            println!("Downloads msys2 and/or mingw64 PACKAGE(s) with their dependencies.");
            println!(
                "Bash and coreutils are automatically installed first because they are required"
            );
            println!("for executing the post-install script of some packages.");
            println!();
            println!("-a, --add    add the packages to an existing install");
            println!("-h, --help   display this help and exit");
            println!();
            exit(0);
        }
        None => {}
    }

    let msys_packages = match get_msys_repo_packages() {
        Some(packages) => packages,
        None => panic!(Color::Red.paint(format!(
            "Failed to get package list from repo {}",
            MSYS_REPO_URL
        ))),
    };
    let mingw64_packages = match get_mingw64_repo_packages() {
        Some(packages) => packages,
        None => panic!(Color::Red.paint(format!(
            "Failed to get package list from repo {}",
            MINGW64_REPO_URL
        ))),
    };

    let append = args.iter().any(|it| *it == "-a" || *it == "--add");
    let pwd = std::env::current_dir().unwrap();
    let target = pwd.join("dist");
    if !append {
        if let Err(_) = rm_rf::ensure_removed(&target) {
            panic!(Color::Red.paint("Failed to remove existing \"dist\" directory"))
        }
    }
    let _ = std::fs::create_dir(&target);
    let _ = std::fs::create_dir(&target.join("tmp"));

    let mut installed: BTreeSet<String> = BTreeSet::new();
    let mut requirements = Requirements::new();
    requirements.add("bash".to_string());
    install(
        &target,
        &mut requirements,
        &mut installed,
        &msys_packages,
        &mingw64_packages,
    );
    installed.insert("sh".to_string());
    let _ = rm_rf::remove(target.join("usr").join("bin").join("bashbug"));

    requirements.add("coreutils".to_string());
    install(
        &target,
        &mut requirements,
        &mut installed,
        &msys_packages,
        &mingw64_packages,
    );

    args.into_iter()
        .filter(|it| !it.starts_with("-") && !installed.contains(it))
        .for_each(|it| requirements.append(it));
    if !requirements.is_empty() {
        install(
            &target,
            &mut requirements,
            &mut installed,
            &msys_packages,
            &mingw64_packages,
        )
    }
}

fn install(
    target: &Path,
    requirements: &mut Requirements,
    installed: &mut BTreeSet<String>,
    msys_packages: &HashMap<String, Package>,
    mingw64_packages: &HashMap<String, Package>,
) {
    while !requirements.is_empty() {
        let name = requirements.take().unwrap();
        println!("{}", Color::Purple.paint(&name));
        // match get_latest_package(&name, mingw64_packages)
        //     .or_else(|| {
        //         get_latest_package(
        //             &format!("{}{}", MINGW64_PKG_PREFIX, &name),
        //             mingw64_packages,
        //         )
        //     })
        //     .or_else(|| get_latest_package(&name, msys_packages))
        match get_latest_package(&name, msys_packages)
            .or_else(|| get_latest_package(&name, &mingw64_packages))
        {
            None => {
                println!(
                    "{}",
                    Color::Red.paint(&format!("Failed to find any package for {}", &name))
                );
            }
            Some(package) => {
                let bytes = download(&package.url);
                let bytes = match package.compression {
                    Compression::ZSTD => decompress_zstd(&bytes),
                    Compression::XZ => decompress_xz(&bytes),
                };
                let dependencies = unpack_package_tar(&target, &bytes);
                for it in dependencies {
                    if !installed.contains(&it) {
                        println!("Adding dependency {}", &it);
                        requirements.add(it);
                    }
                }
                if name.starts_with(MINGW64_PKG_PREFIX) {
                    installed.insert(name.replacen(MINGW64_PKG_PREFIX, "", 1));
                }
                installed.insert(name);
            }
        }
    }
}

fn unpack_package_tar(target: &Path, data: &[u8]) -> Vec<String> {
    println!("Extracting tar archive");
    let mut dependencies = vec![];
    let bash_env_path = target.join("usr").join("bin").to_string_lossy().to_string();
    match tar::Archive::new(data).entries() {
        Ok(entries) => {
            entries.filter_map(|it| it.ok()).for_each(|mut entry| {
                match entry.path() {
                    Ok(name) => {
                        if name.is_relative() {
                            let re = Regex::new("[=>~#*]").unwrap();
                            match name.to_string_lossy().borrow() {
                                ".BUILDINFO" => {}
                                ".MTREE" => {}
                                ".PKGINFO" => {
                                    let path = target.join(name);
                                    entry.unpack(&path).unwrap();
                                    BufReader::new(File::open(&path).unwrap())
                                        .lines()
                                        .filter_map(|line| {
                                            line.ok().and_then(|line| {
                                                let split: Vec<&str> =
                                                    line.split(" = ").into_iter().collect();
                                                if split.len() == 2
                                                    && *split.first().unwrap() == "depend"
                                                {
                                                    let dependency = split.last().unwrap();
                                                    let dependency = re
                                                        .split(dependency)
                                                        .into_iter()
                                                        .next()
                                                        .unwrap();
                                                    Some(dependency.to_string())
                                                } else {
                                                    None
                                                }
                                            })
                                        })
                                        .for_each(|dependency| {
                                            dependencies.push(dependency);
                                        });
                                    std::fs::remove_file(&path).unwrap();
                                }
                                ".INSTALL" => {
                                    let path = target.join(name);
                                    entry.unpack(&path).unwrap();
                                    match std::process::Command::new(
                                        &target.join("usr").join("bin").join("bash.exe"),
                                    )
                                    .current_dir(&target)
                                    .arg(".INSTALL")
                                    .env("PATH", &bash_env_path)
                                    .output()
                                    {
                                        Ok(output) => {
                                            if !output.stderr.is_empty() {
                                                println!(
                                                    "{}",
                                                    Color::Red.paint(String::from_utf8_lossy(
                                                        &output.stderr
                                                    ))
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            println!(
                                                "{}",
                                                Color::Red.paint("Failed to run install script")
                                            );
                                            println!("{:?}", err);
                                        }
                                    };
                                    std::fs::remove_file(&path).unwrap();
                                }
                                name => {
                                    if !name.contains("..") {
                                        let path = if name.starts_with("mingw64/") {
                                            target
                                                .join(name.replacen("mingw64/", "usr/", 1).as_str())
                                        } else {
                                            target.join(name)
                                        };
                                        path.parent().and_then(|parent| {
                                            std::fs::create_dir_all(parent).ok()
                                        });
                                        if let Err(_) = entry.unpack(&path) {
                                            println!(
                                                "{}",
                                                Color::Red.paint(&format!(
                                                    "Failed to extract {}",
                                                    path.strip_prefix(&target)
                                                        .unwrap()
                                                        .to_string_lossy()
                                                ))
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => println!("{}", &Color::Red.paint("Invalid path in tar archive")),
                };
            });
        }
        Err(_) => panic!(Color::Red.paint("Failed to decompress tar archive")),
    }
    return dependencies;
}

fn decompress_zstd(data: &[u8]) -> Vec<u8> {
    println!("Decompressing zstd archive");
    match zstd::decode_all(data) {
        Ok(bytes) => bytes,
        Err(_) => panic!(Color::Red.paint("Failed to decompress zstd archive")),
    }
}

fn decompress_xz(data: &[u8]) -> Vec<u8> {
    println!("Decompressing xz archive");
    match xz_decom::decompress(data) {
        Ok(bytes) => bytes,
        Err(_) => panic!(Color::Red.paint("Failed to decompress xz archive")),
    }
}

fn download(url: &str) -> Vec<u8> {
    println!("Downloading {}", url);
    match reqwest::blocking::get(url)
        .and_then(|resp| resp.bytes())
        .map(|bytes| bytes.to_vec())
    {
        Ok(url) => url,
        Err(_) => panic!(Color::Red.paint(format!("Failed to download {}", url))),
    }
}

fn get_latest_package<'a>(
    name: &'_ str,
    packages: &'a HashMap<String, Package>,
) -> Option<&'a Package> {
    let prefix = format!("{}-", name);
    let re = Regex::new("^[0-9]+.*$").unwrap();
    let mut links: Vec<_> = packages
        .iter()
        .filter_map(|it| {
            let (name_and_version, package) = it;
            if name_and_version.starts_with(&prefix) {
                let version = name_and_version.replacen(&prefix, "", 1);
                if re.is_match(&version) {
                    Some((version, package))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    links.sort_by(|a, b| a.0.cmp(&b.0));
    links.into_iter().last().and_then(|it| {
        println!("{}", it.0);
        Some(it.1)
    })
}

fn get_msys_repo_packages() -> Option<HashMap<String, Package>> {
    get_repo_packages("msys", MSYS_REPO_URL)
}

fn get_mingw64_repo_packages() -> Option<HashMap<String, Package>> {
    get_repo_packages("mingw64", MINGW64_REPO_URL)
}

fn get_repo_packages(repo_name: &str, repo_url: &str) -> Option<HashMap<String, Package>> {
    println!("Fetching {} package list", repo_name);
    let html = match reqwest::blocking::get(repo_url).and_then(|it| it.text()) {
        Ok(text) => text,
        Err(_) => return None,
    };
    let document = Html::parse_document(&html);
    let table = match document
        .select(&Selector::parse("body>table").unwrap())
        .next()
    {
        Some(element) => element,
        None => return None,
    };
    let td_selector = Selector::parse("td").unwrap();
    let a_selector = Selector::parse("a").unwrap();
    let re = Regex::new("^(.*)-(?:x86_64|any)[.]pkg[.]tar[.](xz|zst)$").unwrap();
    Some(
        table
            .select(&Selector::parse("tr").unwrap())
            .into_iter()
            .skip(1)
            .filter_map(|row| {
                row.select(&td_selector).skip(1).next().and_then(|td| {
                    td.select(&a_selector).next().and_then(|a| {
                        let text = a.inner_html().trim().to_string();
                        re.captures(&text).and_then(|captures| {
                            a.value().attr("href").and_then(|url| {
                                let name = captures.get(1).unwrap().as_str().to_string();
                                let extension = match captures.get(2).unwrap().as_str() {
                                    "zst" => Some(Compression::ZSTD),
                                    "xz" => Some(Compression::XZ),
                                    _ => None,
                                };
                                extension.and_then(|compression| {
                                    Some((
                                        name,
                                        Package {
                                            url: format!("{}{}", repo_url, url),
                                            compression,
                                        },
                                    ))
                                })
                            })
                        })
                    })
                })
            })
            .collect(),
    )
}
