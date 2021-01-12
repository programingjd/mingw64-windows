extern crate lazy_static;

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::commands::errors::Error::{DecompressionError, DownloadError};
use crate::commands::errors::Result;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum YesNoAnswer {
    YES,
    NO,
}

pub fn yes_or_no(
    question: &str,
    default: YesNoAnswer,
    no_prompt: bool,
    no_prompt_message: Option<&str>,
) -> YesNoAnswer {
    if no_prompt {
        if let Some(message) = no_prompt_message {
            println!("{}", message);
        }
        return default;
    }
    prompt(question, default);
    let mut line = String::new();
    loop {
        let _ = io::stdin().read_line(&mut line);
        match line.trim() {
            "y" | "yes" | "Y" | "Yes" | "YES" => break YesNoAnswer::YES,
            "n" | "no" | "N" | "No" | "NO" => break YesNoAnswer::NO,
            "" => break default,
            _ => {
                line.clear();
                prompt(question, default);
            }
        };
    }
}

fn prompt(question: &str, default: YesNoAnswer) {
    match default {
        YesNoAnswer::YES => println!("{} (Y/n)", question),
        YesNoAnswer::NO => println!("{} (y/N)", question),
    }
}

pub fn text_input(header: &str, default: Option<&str>) -> String {
    match default {
        Some(default) => println!("{} ({})", header, default),
        None => println!("{}", header),
    }
    let mut line = String::new();
    loop {
        let _ = io::stdin().read_line(&mut line);
        match line.trim() {
            "" => {
                if let Some(default) = default {
                    break default;
                } else {
                    line.clear();
                }
            }
            it => break it,
        };
    }
    .to_string()
}

/// Looks at the last modification date and returns true if less than an hour has elapsed.
/// Returns false if an error occurs (e.g. the file doesn't exist).
pub fn file_was_updated_recently(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map(|time| {
            time.elapsed()
                .map(|elapsed| elapsed < Duration::from_secs(3600))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// A string representing the value of http response header: 'ETag'.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct ETag {
    pub value: String,
}

impl Display for ETag {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.value)
    }
}

impl From<String> for ETag {
    fn from(value: String) -> Self {
        Self { value }
    }
}

impl From<&str> for ETag {
    fn from(value: &str) -> Self {
        Self {
            value: value.to_string(),
        }
    }
}

/// The ETag header sent by server is used to check if we already have the latest version.
#[derive(Debug)]
pub struct Response {
    pub etag: ETag,
    pub body: Vec<u8>,
}

/// Performs a HEAD request to get the ETag header.
pub fn etag(url: &str) -> Result<ETag> {
    reqwest::blocking::Client::new()
        .head(url)
        .send()
        .ok()
        .and_then(|resp| {
            resp.headers()
                .get("etag")
                .and_then(|value| value.to_str().ok().map(|it| ETag::from(it)))
        })
        .ok_or(DownloadError)
}

pub fn download(url: &str) -> Result<Response> {
    reqwest::blocking::get(url)
        .ok()
        .and_then(|resp| {
            resp.headers()
                .get("etag")
                .and_then(|it| it.to_str().map(|it| it.to_string()).ok())
                .and_then(|etag| {
                    resp.bytes().ok().map(|it| Response {
                        etag: ETag::from(etag),
                        body: it.to_vec(),
                    })
                })
        })
        .ok_or(DownloadError)
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub enum Compression {
    ZSTD,
    XZ,
    GZ,
}

lazy_static! {
    static ref ALL_COMPRESSIONS: Vec<&'static Compression> =
        vec![&Compression::ZSTD, &Compression::XZ, &Compression::GZ];
}

impl Compression {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::ZSTD => "zst",
            Self::XZ => "xz",
            Self::GZ => "gz",
        }
    }
    pub fn from_extension(extension: &str) -> Option<&'static Self> {
        ALL_COMPRESSIONS
            .iter()
            .find(|&it| it.extension() == extension)
            .map(|it| *it)
    }
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::ZSTD => Self::decompress_zstd(data),
            Self::XZ => Self::decompress_lzma(data),
            Self::GZ => Self::decompress_gzip(data),
        }
    }
    fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data).map_err(|_| DecompressionError)
    }
    fn decompress_lzma(data: &[u8]) -> Result<Vec<u8>> {
        xz_decom::decompress(data).map_err(|_| DecompressionError)
    }
    fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>> {
        // skip the 10 bytes header: 0x1f (magic), 0x8b (deflate), 4 bytes timestamp, 4 bytes flags
        inflate::inflate_bytes(&data[10..]).map_err(|_| DecompressionError)
    }
}

impl Display for Compression {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.extension())
    }
}
