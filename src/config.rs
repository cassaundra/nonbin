use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub base_url: String,
    pub port: u16,
    pub database: Database,
    pub storage: Storage,
    pub limits: Limits,
    pub word_lists: WordLists,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Database {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Storage {
    pub kind: StorageKind,
    pub file: FileStorage,
    #[cfg(feature = "s3")]
    pub s3: S3Storage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileStorage {
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg(feature = "s3")]
pub struct S3Storage {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageKind {
    File,
    #[cfg(feature = "s3")]
    S3,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Limits {
    pub max_upload_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WordLists {
    pub adjectives_file: PathBuf,
    pub nouns_file: PathBuf,
}
