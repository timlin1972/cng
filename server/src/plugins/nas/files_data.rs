use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::utils;

#[derive(Debug, Deserialize, Serialize)]
pub struct FileData {
    pub filename: String,
    pub md5: String,
    pub modified: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilesData {
    pub files_data: Vec<FileData>,
}

pub fn get_files_data_recursive(path: &Path, files_data: &mut Vec<FileData>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let filename = path.to_string_lossy().to_string();
                let modified = fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .map(|time| time.duration_since(UNIX_EPOCH))
                    .map(|dur| dur.unwrap().as_secs())
                    .unwrap_or(0);

                let md5 = utils::calculate_md5(&filename).unwrap();

                files_data.push(FileData {
                    filename,
                    md5,
                    modified,
                });
            } else if path.is_dir() {
                get_files_data_recursive(&path, files_data);
            }
        }
    }
}

pub fn get_files_data(path: &Path) -> FilesData {
    let mut files_data = vec![];

    get_files_data_recursive(path, &mut files_data);

    FilesData { files_data }
}
