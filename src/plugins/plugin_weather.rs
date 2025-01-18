use async_trait::async_trait;
use chrono::NaiveDateTime;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, City, Cmd, Data, Msg};
use crate::plugins::plugins_main;
use crate::utils;

pub const NAME: &str = "weather";
const WEATHER_POLLING: u64 = 60 * 60;

fn datetime_str_to_ts(datetime_str: &str) -> i64 {
    let naive_datetime = NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M")
        .expect("解析日期時間字串失敗");
    naive_datetime.and_utc().timestamp()
}

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    weather: Vec<City>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let weather = vec![
            City {
                name: "Xindian".to_owned(),
                latitude: 24.9676,
                longitude: 121.542,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Xinzhuang".to_owned(),
                latitude: 25.0359,
                longitude: 121.45,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Taipei".to_owned(),
                latitude: 25.0330,
                longitude: 121.5654,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Tainan".to_owned(),
                latitude: 23.1725,
                longitude: 120.279,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Eindhoven".to_owned(),
                latitude: 51.44,
                longitude: 5.46,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Tokyo".to_owned(),
                latitude: 35.6895,
                longitude: 139.6917,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Seattle".to_owned(),
                latitude: 47.6062,
                longitude: 122.3321,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Chiang Mai".to_owned(),
                latitude: 18.7061,
                longitude: 98.9817,
                ts: None,
                temperature: None,
                code: None,
            },
            City {
                name: "Pai".to_owned(),
                latitude: 19.3583,
                longitude: 98.4418,
                ts: None,
                temperature: None,
                code: None,
            },
        ];

        Self {
            name: NAME.to_owned(),
            msg_tx,
            weather,
        }
    }

    async fn init(&mut self) {
        let msg_tx_clone = self.msg_tx.clone();
        let weather = self.weather.clone();
        tokio::spawn(async move {
            loop {
                for city in &weather {
                    let weather = utils::weather(city.latitude, city.longitude).await;
                    msg::cmd(
                        &msg_tx_clone,
                        cfg::name(),
                        NAME.to_owned(),
                        msg::ACT_WEATHER.to_owned(),
                        vec![
                            city.name.to_owned(),
                            weather.time.to_owned(),
                            weather.temperature.to_string(),
                            weather.code.to_string(),
                        ],
                    )
                    .await;
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(WEATHER_POLLING)).await;
            }
        });

        log(&self.msg_tx, cfg::name(), Trace, format!("[{NAME}] init")).await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        for city in &self.weather {
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("[{NAME}] {}:", city.name),
            )
            .await;

            let datetime = match city.ts {
                None => "n/a".to_owned(),
                Some(t) => utils::ts_str(t as u64),
            };
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("[{NAME}]     Datetime: {datetime}"),
            )
            .await;

            let temperature = match city.temperature {
                None => "n/a".to_owned(),
                Some(t) => t.to_string(),
            };
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("[{NAME}]     Temperature: {temperature}°C"),
            )
            .await;

            let code = match city.code {
                None => "n/a".to_owned(),
                Some(c) => utils::weather_code_str(c).to_owned(),
            };
            log(
                &self.msg_tx,
                cmd.reply.clone(),
                Info,
                format!("[{NAME}]     Weather: {code}"),
            )
            .await;
        }
    }

    async fn update(&mut self, cmd: &Cmd) {
        for city in &mut self.weather {
            let weather = utils::weather(city.latitude, city.longitude).await;
            city.ts = Some(datetime_str_to_ts(&weather.time));
            city.temperature = Some(weather.temperature);
            city.code = Some(weather.code);
        }

        msg::weather(&self.msg_tx, self.weather.clone()).await;
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!("[{NAME}] update"),
        )
        .await;
    }

    async fn weather(&mut self, cmd: &Cmd) {
        let city_name = cmd.data[0].clone();
        let city = self
            .weather
            .iter_mut()
            .find(|p| p.name == city_name)
            .unwrap_or_else(|| panic!("City not found: {}", city_name));

        city.ts = Some(datetime_str_to_ts(&cmd.data[1]));
        city.temperature = Some(cmd.data[2].parse::<f32>().unwrap());
        city.code = Some(cmd.data[3].parse::<u8>().unwrap());

        msg::weather(&self.msg_tx, self.weather.clone()).await;
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
                msg::ACT_WEATHER => self.weather(cmd).await,
                msg::ACT_UPDATE => self.update(cmd).await,
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
