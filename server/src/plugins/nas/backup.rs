use std::fs;
use std::path::Path;

use chrono::NaiveDate;
use log::Level::Info;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;

use crate::cfg;
use crate::info;
use crate::msg::{log, Msg, Reply};

pub const NAME: &str = "nas";

const BACKUP_DIR: &str = "./backup";

pub fn backup(msg_tx_clone: Sender<Msg>) {
    tokio::spawn(async move {
        loop {
            // check if backup is needed
            // backup dir is BACKUP_DIR+current_date (e.g. ./backup/2023-10-01)
            let now = chrono::Local::now();
            let date = now.format("%Y-%m-%d").to_string();
            let backup_dir = format!("{BACKUP_DIR}/{date}");
            if !Path::new(&backup_dir).exists() {
                fs::create_dir_all(&backup_dir).unwrap();

                // copy all files from cfg::FILE_FOLDER to backup_dir recursively
                let files = get_all_files_recursively(Path::new(cfg::FILE_FOLDER));
                for file in &files {
                    let src = Path::new(file);
                    let dst =
                        Path::new(&backup_dir).join(src.strip_prefix("./shared").unwrap_or(src));
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent).unwrap();
                    }
                    fs::copy(src, dst).unwrap();
                }

                info!(
                    &msg_tx_clone,
                    format!("[{NAME}] Backup created: {backup_dir}")
                );

                // we keep at most 7 days of backup
                let keep_latest_n = 7;
                let mut date_dirs: Vec<(NaiveDate, String)> = fs::read_dir(BACKUP_DIR)
                    .unwrap()
                    .filter_map(|entry| {
                        let entry = entry.ok().unwrap();
                        let name = entry.file_name().to_string_lossy().into_owned();

                        match NaiveDate::parse_from_str(&name, "%Y-%m-%d") {
                            Ok(date) => Some((date, name)),
                            Err(_) => None,
                        }
                    })
                    .collect();

                date_dirs.sort_by_key(|(date, _)| *date);

                if date_dirs.len() > keep_latest_n {
                    let to_delete = &date_dirs[..date_dirs.len() - keep_latest_n];
                    for (_, name) in to_delete {
                        let path = Path::new(BACKUP_DIR).join(name);
                        if path.is_dir() {
                            info!(
                                &msg_tx_clone,
                                format!("[{NAME}] Backup removed: {}", path.display())
                            );
                            fs::remove_dir_all(path).unwrap();
                        }
                    }
                }
            }

            // sleep for 4 hours
            tokio::time::sleep(Duration::from_secs(4 * 60 * 60)).await;
        }
    });
}

fn get_all_files_recursively(path: &Path) -> Vec<String> {
    let mut output = Vec::new();

    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let full_path = path.to_string_lossy().to_string();
                output.push(full_path);
            }
        }
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                output.extend(get_all_files_recursively(&path));
            }
        }
    }

    output
}
