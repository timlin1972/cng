use std::process::Stdio;

use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, Msg};
use crate::plugins::plugins_main;

pub const NAME: &str = "shell";

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    child: Option<Child>,
    stdin: Option<tokio::process::ChildStdin>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            child: None,
            stdin: None,
        }
    }

    async fn init(&mut self) {
        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;
    }

    async fn stdout_task(&mut self, child: &mut Child, cmd: &Cmd) {
        let stdout = child.stdout.take().expect("Failed to open stdout");

        let mut reader = BufReader::new(stdout).lines();
        let reply = cmd.reply.to_owned();
        let msg_tx = self.msg_tx.clone();
        tokio::spawn(async move {
            while let Some(line) = reader.next_line().await.unwrap() {
                log(&msg_tx, reply.to_owned(), Info, line).await;
            }
        });
    }

    async fn stderr_task(&mut self, child: &mut Child, cmd: &Cmd) {
        let stderr = child.stderr.take().expect("Failed to open stdout");
        let mut reader = BufReader::new(stderr).lines();
        let reply = cmd.reply.to_owned();
        let msg_tx = self.msg_tx.clone();
        tokio::spawn(async move {
            while let Some(line) = reader.next_line().await.unwrap() {
                log(&msg_tx, reply.to_owned(), Info, line).await;
            }
        });
    }

    async fn start(&mut self, cmd: &Cmd) {
        let mut child = Command::new(cfg::shell())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start shell");

        self.stdout_task(&mut child, cmd).await;
        self.stderr_task(&mut child, cmd).await;

        self.stdin = Some(child.stdin.take().expect("Failed to open stdin"));
        self.child = Some(child);

        log(
            &self.msg_tx,
            cmd.reply.to_owned(),
            Info,
            format!("[{NAME}] shell start"),
        )
        .await;
    }

    async fn cmd(&mut self, cmd: &Cmd) {
        if self.child.is_none() {
            log(
                &self.msg_tx,
                cmd.reply.to_owned(),
                Error,
                format!("[{NAME}] cmd: child is none"),
            )
            .await;
            return;
        }

        if self.stdin.is_none() {
            log(
                &self.msg_tx,
                cmd.reply.to_owned(),
                Error,
                format!("[{NAME}] cmd: stdin is none"),
            )
            .await;
            return;
        }

        let shell_cmd = format!("{}\n", cmd.data[0]);
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(shell_cmd.as_bytes())
            .await
            .unwrap();
    }

    async fn stop(&mut self, cmd: &Cmd) {
        if self.child.is_none() {
            log(
                &self.msg_tx,
                cmd.reply.to_owned(),
                Error,
                format!("[{NAME}] cmd: child is none"),
            )
            .await;
            return;
        }

        if self.stdin.is_none() {
            log(
                &self.msg_tx,
                cmd.reply.to_owned(),
                Error,
                format!("[{NAME}] cmd: stdin is none"),
            )
            .await;
            return;
        }

        let shell_cmd = "exit\n".to_owned();
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(shell_cmd.as_bytes())
            .await
            .unwrap();
        self.stdin.as_mut().unwrap().shutdown().await.unwrap();

        self.child.as_mut().unwrap().wait().await.unwrap();

        self.stdin = None;
        self.child = None;

        log(
            &self.msg_tx,
            cmd.reply.to_owned(),
            Info,
            format!("[{NAME}] shell stop"),
        )
        .await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "shell: {}",
                if self.child.is_some() {
                    "running"
                } else {
                    "stopped"
                }
            ),
        )
        .await;
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "stdin: {}",
                if self.stdin.is_some() {
                    "open"
                } else {
                    "closed"
                }
            ),
        )
        .await;
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
                msg::ACT_START => self.start(cmd).await,
                msg::ACT_CMD => self.cmd(cmd).await,
                msg::ACT_STOP => self.stop(cmd).await,
                msg::ACT_SHOW => self.show(cmd).await,
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
