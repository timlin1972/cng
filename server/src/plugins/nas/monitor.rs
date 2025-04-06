use std::path::Path;
use std::{collections::HashMap, sync::Arc};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::{self, Sender};
use tokio::task;
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};

use log::Level::{Error, Info};

use crate::cfg;
use crate::msg::{self, log, Msg, Reply};
use crate::utils;
use crate::{error, info, reply_me, unknown};

pub const NAME: &str = "nas";

const DEBOUNCE_DELAY: u64 = 10; // seconds

type DebounceMap = Arc<Mutex<HashMap<(String, EventKind), tokio::task::JoinHandle<()>>>>;

pub fn monitor(msg_tx: Sender<Msg>) {
    tokio::spawn(async move {
        let debounce_map: DebounceMap = Arc::new(Mutex::new(HashMap::new()));

        let path_to_watch = Path::new(cfg::FILE_FOLDER);

        let (tx, mut rx) = mpsc::channel(1024);

        let _watcher_handle = task::spawn_blocking(move || {
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    if let Ok(event) = res {
                        let _ = tx.blocking_send(event);
                    }
                },
                Config::default(),
            )
            .expect("Watcher 初始化失敗");

            watcher
                .watch(Path::new(path_to_watch), RecursiveMode::Recursive)
                .expect("無法監聽目錄");

            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });

        info!(&msg_tx, format!("[{NAME}] Monitoring {path_to_watch:?}"));

        while let Some(event) = rx.recv().await {
            for path in &event.paths {
                let path_str = path.display().to_string();
                let debounce_map = debounce_map.clone();

                let key = (path_str.clone(), event.kind);

                // cancel the previous task if it exists
                let mut map = debounce_map.lock().await;
                if let Some(handle) = map.remove(&key) {
                    handle.abort(); // Abort the previous task
                }

                let event_clone = event.clone(); // Clone the event
                let msg_tx_clone = msg_tx.clone(); // Clone the message sender

                // spawn a new task with a debounce delay
                let handle = tokio::spawn(async move {
                    sleep(Duration::from_secs(DEBOUNCE_DELAY)).await;
                    handle_event(event_clone, &msg_tx_clone).await;
                });

                // store the new task handle in the map
                map.insert(key, handle);
            }
        }
    });
}

fn monitor_get_file(file_path: &str) -> String {
    let keyword = "./shared/";
    if let Some(pos) = file_path.find(keyword) {
        let result = &file_path[pos..];
        return result.to_owned();
    }

    "".to_owned()
}

async fn handle_event(event: Event, msg_tx_clone: &Sender<Msg>) {
    match event.kind {
        notify::event::EventKind::Create(_) => (),
        notify::event::EventKind::Modify(_) => {
            for path in event.paths.iter() {
                let filename = monitor_get_file(path.to_str().unwrap());

                info!(
                    msg_tx_clone,
                    format!("[{NAME}] [monitor] File is modified: {filename}")
                );

                msg::cmd(
                    msg_tx_clone,
                    reply_me!(),
                    NAME.to_owned(),
                    msg::ACT_NAS.to_owned(),
                    vec![
                        "remote_modify".to_owned(),
                        filename.to_owned(),
                        utils::ts().to_string(),
                    ],
                )
                .await;
            }
        }
        notify::event::EventKind::Remove(_) => {
            for path in event.paths.iter() {
                let filename = monitor_get_file(path.to_str().unwrap());

                info!(
                    msg_tx_clone,
                    format!("[{NAME}] [monitor] File is removed: {filename}")
                );

                msg::cmd(
                    msg_tx_clone,
                    reply_me!(),
                    NAME.to_owned(),
                    msg::ACT_NAS.to_owned(),
                    vec!["remote_remove".to_owned(), filename.to_owned()],
                )
                .await;
            }
        }
        notify::event::EventKind::Access(_) => (),
        _ => {
            unknown!(msg_tx_clone, NAME, event);
        }
    }
}
