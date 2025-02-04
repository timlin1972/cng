use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;

use crate::msg::{self, log, Cmd, Data, Msg, Worldtime};
use crate::plugins::plugins_main;
use crate::{cfg, utils};

pub const NAME: &str = "worldtime";
const WORLDTIME_POLLING: u64 = 5 * 60;

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    cities: Vec<Worldtime>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            cities: vec![
                Worldtime::new("Taipei".to_owned(), "Asia/Taipei".to_owned()),
                Worldtime::new("Tokyo".to_owned(), "Asia/Tokyo".to_owned()),
                Worldtime::new("Bangkok".to_owned(), "Asia/Bangkok".to_owned()),
                Worldtime::new("Amsterdam".to_owned(), "Europe/Amsterdam".to_owned()),
                Worldtime::new("Seattle".to_owned(), "America/Los_Angeles".to_owned()),
            ],
        }
    }

    async fn init(&mut self) {
        let msg_tx_clone = self.msg_tx.clone();
        let cities = self.cities.clone();
        tokio::spawn(async move {
            loop {
                for city in &cities {
                    let datetime = utils::get_city_time(&city.timezone).await;
                    if let Ok(datetime) = datetime {
                        msg::cmd(
                            &msg_tx_clone,
                            cfg::name(),
                            NAME.to_owned(),
                            msg::ACT_WORLDTIME.to_owned(),
                            vec![
                                city.name.clone(),
                                utils::convert_datetime(&datetime).unwrap(),
                            ],
                        )
                        .await;
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(WORLDTIME_POLLING)).await;
            }
        });

        log(&self.msg_tx, cfg::name(), Info, format!("[{NAME}] init")).await;
    }

    async fn update(&mut self, cmd: &Cmd) {
        let msg_tx_clone = self.msg_tx.clone();
        let cities = self.cities.clone();
        let reply_clone = cmd.reply.clone();
        tokio::spawn(async move {
            for city in &cities {
                let datetime = utils::get_city_time(&city.timezone).await;
                if let Ok(datetime) = datetime {
                    msg::cmd(
                        &msg_tx_clone,
                        cfg::name(),
                        NAME.to_owned(),
                        msg::ACT_WORLDTIME.to_owned(),
                        vec![
                            city.name.clone(),
                            utils::convert_datetime(&datetime).unwrap(),
                        ],
                    )
                    .await;
                }
            }

            log(&msg_tx_clone, reply_clone, Info, format!("[{NAME}] update")).await;
        });
    }

    async fn show(&mut self, cmd: &Cmd) {
        for city in &self.cities {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("{:15}: {}", city.name, city.datetime),
            )
            .await;
        }
    }

    async fn worldtime(&mut self, cmd: &Cmd) {
        let city = cmd.data[0].clone();
        let datetime = cmd.data[1].clone();
        for c in &mut self.cities {
            if c.name == *city {
                c.datetime = datetime.clone();
            }
        }

        msg::worldtime(&self.msg_tx, self.cities.clone()).await;
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
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_UPDATE => self.update(cmd).await,
                msg::ACT_WORLDTIME => self.worldtime(cmd).await,
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
