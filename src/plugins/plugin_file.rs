use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::Path;

use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, Msg};
use crate::plugins::plugins_main;

pub const NAME: &str = "file";
const FILE_FOLDER: &str = "./shared";
const BUFFER_SIZE: usize = 4 * 1024;

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    filename: Option<String>,
    sequence: usize,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            filename: None,
            sequence: 0,
        }
    }

    async fn init(&mut self) {
        if !Path::new(FILE_FOLDER).exists() {
            fs::create_dir(FILE_FOLDER).unwrap();
            log(
                &self.msg_tx,
                cfg::name(),
                Info,
                format!("[{NAME}] Folder '{FILE_FOLDER}' is created."),
            )
            .await;
        } else {
            log(
                &self.msg_tx,
                cfg::name(),
                Info,
                format!("[{NAME}] Folder '{FILE_FOLDER}' is existed."),
            )
            .await;
        }

        log(&self.msg_tx, cfg::name(), Info, format!("[{NAME}] init")).await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] filename: {:?}", self.filename),
        )
        .await;

        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] sequence: {}", self.sequence),
        )
        .await;

        // list files in shared file
        let paths = fs::read_dir(FILE_FOLDER).unwrap();
        for path in paths {
            let path = path.unwrap().path();
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("[{NAME}] file: {:?}", path.file_name().unwrap()),
            )
            .await;
        }
    }

    async fn stop(&mut self, cmd: &Cmd) {
        self.filename = None;
        self.sequence = 0;

        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] stop"),
        )
        .await;
    }

    async fn put(&mut self, cmd: &Cmd) {
        if cmd.reply == cfg::name() {
            log(
                &self.msg_tx,
                cfg::name(),
                Error,
                format!("[{NAME}] put is not for local use."),
            )
            .await;
            return;
        }

        let path = format!("{FILE_FOLDER}/{}", cmd.data[0]);

        // check if file exist or not
        if !Path::new(&path).exists() {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Error,
                format!("[{NAME}] file not found: {:?}", cmd.data[0]),
            )
            .await;
            return;
        }

        // get file size
        let file = File::open(path).unwrap();
        let metadata = file.metadata().unwrap();
        let size = metadata.len();
        let sequence = if size % BUFFER_SIZE as u64 == 0 {
            (size / BUFFER_SIZE as u64) as usize
        } else {
            (size / BUFFER_SIZE as u64) as usize + 1
        };

        msg::file_filename(
            &self.msg_tx,
            cmd.reply.clone(),
            cmd.data[0].clone(),
            sequence,
        )
        .await;

        let mut reader = BufReader::new(file);

        let mut buffer = [0; BUFFER_SIZE];

        let mut sequence = 0;
        loop {
            let bytes_read = reader.read(&mut buffer).unwrap();

            if bytes_read == 0 {
                break;
            }

            msg::file_content(
                &self.msg_tx,
                cmd.reply.clone(),
                sequence,
                &buffer[..bytes_read],
            )
            .await;

            sequence += 1;
        }

        msg::file_end(&self.msg_tx, cmd.reply.clone(), sequence).await;
    }

    async fn file(&mut self, cmd: &Cmd) {
        match cmd.data.first() {
            Some(data) => match data.as_str() {
                "filename" => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!(
                            "[{NAME}] file: filename: {:?}, sequence: {}",
                            cmd.data[1], cmd.data[2]
                        ),
                    )
                    .await;

                    self.filename = Some(cmd.data[1].clone());
                    self.sequence = 0;

                    let path = format!("{FILE_FOLDER}/{}", self.filename.as_ref().unwrap());
                    let _ = File::create(path).unwrap();
                }
                "content" => {
                    if self.filename.is_none() {
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Error,
                            format!("[{NAME}] file: no filename"),
                        )
                        .await;
                        return;
                    }

                    let sequence = cmd.data[1].parse::<usize>().unwrap();
                    if sequence != self.sequence {
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Error,
                            format!("[{NAME}] file: invalid sequence: {:?}", sequence),
                        )
                        .await;
                        return;
                    }

                    let path = format!("{FILE_FOLDER}/{}", self.filename.as_ref().unwrap());
                    let mut file = OpenOptions::new().append(true).open(path).unwrap();

                    let content = ascii85::decode(&cmd.data[2]).unwrap();
                    file.write_all(&content).unwrap();

                    self.sequence += 1;

                    log(
                        &self.msg_tx,
                        cfg::name(),
                        Info,
                        format!("[{NAME}] file: content: {:?}", sequence),
                    )
                    .await;
                }
                "end" => {
                    if self.filename.is_none() {
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Error,
                            format!("[{NAME}] file: no filename"),
                        )
                        .await;
                        return;
                    }

                    let sequence = cmd.data[1].parse::<usize>().unwrap();
                    if sequence != self.sequence {
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Error,
                            format!("[{NAME}] file: invalid sequence: {:?}", sequence),
                        )
                        .await;
                        return;
                    }

                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!("[{NAME}] file: end: {:?}", sequence),
                    )
                    .await;

                    self.filename = None;
                    self.sequence = 0;
                }
                _ => {
                    log(
                        &self.msg_tx,
                        cfg::name(),
                        Error,
                        format!("[{NAME}] file: invalid data: {:?}", data),
                    )
                    .await;
                }
            },
            None => {
                log(
                    &self.msg_tx,
                    cfg::name(),
                    Error,
                    format!("[{NAME}] file: no data"),
                )
                .await;
            }
        }
    }
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn msg(&mut self, msg: &Msg) -> bool {
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                msg::ACT_INIT => self.init().await,
                msg::ACT_PUT => self.put(cmd).await,
                msg::ACT_FILE => self.file(cmd).await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_STOP => self.stop(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        cfg::name(),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
            _ => {
                log(
                    &self.msg_tx,
                    cfg::name(),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}
