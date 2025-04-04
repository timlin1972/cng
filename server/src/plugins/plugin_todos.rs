use async_trait::async_trait;
use futures::stream::StreamExt;
use log::Level::{Error, Info};
use mongodb::{
    bson::{doc, DateTime},
    Client,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, Cmd, Data, Msg, Reply};
use crate::plugins::mongodb::utils;
use crate::plugins::plugins_main;
use crate::{error, info, init, unknown};

pub const NAME: &str = "todos";

#[derive(Serialize, Deserialize, Debug)]
struct Todo {
    title: String,
    desc: String,
    priority: i32,
    due: DateTime,
    completed: bool,
    created: DateTime,
    updated: DateTime,
}

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    client: Option<Client>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            client: None,
        }
    }

    async fn init(&mut self) {
        match utils::connect(&cfg::db()).await {
            Ok(t) => {
                self.client = Some(t);
                info!(&self.msg_tx, format!("[{NAME}] DB connected"));
            }
            Err(e) => {
                self.client = None;
                error!(
                    &self.msg_tx,
                    format!("[{NAME}] Failed to connect to DB: {:?}", e)
                );
            }
        }

        init!(&self.msg_tx, NAME);
    }

    async fn show(&mut self, cmd: &Cmd) {
        if self.client.is_none() {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Error,
                format!("[{NAME}] DB not connected"),
            )
            .await;
            return;
        }

        let client = self.client.as_ref().unwrap();
        let collection: mongodb::Collection<Todo> = client.database("cng").collection("todos");
        // let filter = doc! { "title": "title test" };
        let filter = doc! {};
        let mut cursor = collection.find(filter).await.unwrap();
        while let Some(result) = cursor.next().await {
            match result {
                Ok(document) => match &cmd.reply {
                    Reply::Device(_) => {
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("[{}]", document.title),
                        )
                        .await;
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("    desc: {}", document.desc),
                        )
                        .await;
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("    completed: {}", document.completed),
                        )
                        .await;
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("    priority: {}", document.priority),
                        )
                        .await;
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("    due: {}", document.due),
                        )
                        .await;
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("    created: {}", document.created),
                        )
                        .await;
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!("    updated: {}", document.updated),
                        )
                        .await;
                    }
                    Reply::Web(sender) => {
                        sender
                            .send(serde_json::to_value(document).unwrap())
                            .await
                            .unwrap();
                    }
                },
                Err(e) => {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Error,
                        format!("[{NAME}] Failed to find document: {:?}", e),
                    )
                    .await;
                }
            }
        }
    }

    async fn add(&mut self, cmd: &Cmd) {
        if self.client.is_none() {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Error,
                format!("[{NAME}] DB not connected"),
            )
            .await;
            return;
        }

        let title = cmd.data.first().unwrap();
        let desc = cmd.data.get(1).unwrap();
        let priority = cmd.data.get(2).unwrap().parse::<i32>().unwrap();

        let client = self.client.as_ref().unwrap();
        let collection: mongodb::Collection<Todo> = client.database("cng").collection("todos");
        let todo = Todo {
            title: title.to_owned(),
            desc: desc.to_owned(),
            priority,
            due: DateTime::now(),
            completed: false,
            created: DateTime::now(),
            updated: DateTime::now(),
        };
        collection.insert_one(todo).await.unwrap();
    }

    async fn help(&self) {}
}

#[async_trait]
impl plugins_main::Plugin for Plugin {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    async fn msg(&mut self, msg: &Msg) -> bool {
        match &msg.data {
            Data::Cmd(cmd) => match cmd.action.as_str() {
                msg::ACT_HELP => self.help().await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_ADD => self.add(cmd).await,
                _ => {
                    unknown!(&self.msg_tx, NAME, cmd.action);
                }
            },
            _ => {
                unknown!(&self.msg_tx, NAME, msg);
            }
        }

        false
    }
}
