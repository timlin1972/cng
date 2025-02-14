use async_trait::async_trait;
use log::Level::{Error, Info, Trace};
use tokio::sync::mpsc::Sender;

use crate::cfg;
use crate::msg::{self, log, City, Cmd, Data, Msg, Reply};
use crate::plugins::plugins_main;
use crate::utils::{self, Weather, WeatherDaily};

pub const NAME: &str = "weather";
const WEATHER_POLLING: u64 = 60 * 60;

async fn update_weather(msg_tx: &Sender<Msg>, city_name: &str, weather: Weather) {
    msg::cmd(
        msg_tx,
        Reply::Device(cfg::name()),
        NAME.to_owned(),
        msg::ACT_WEATHER.to_owned(),
        vec![
            city_name.to_owned(),
            weather.time.to_owned(),
            weather.temperature.to_string(),
            weather.weathercode.to_string(),
        ],
    )
    .await;

    for (idx, daily) in weather.daily.iter().enumerate() {
        msg::cmd(
            msg_tx,
            Reply::Device(cfg::name()),
            NAME.to_owned(),
            msg::ACT_WEATHER_DAILY.to_owned(),
            vec![
                city_name.to_owned(),
                idx.to_string(),
                daily.time.to_owned(),
                daily.temperature_2m_max.to_string(),
                daily.temperature_2m_min.to_string(),
                daily.precipitation_probability_max.to_string(),
                daily.weather_code.to_string(),
            ],
        )
        .await;
    }
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
                weather: None,
            },
            City {
                name: "Xinzhuang".to_owned(),
                latitude: 25.0359,
                longitude: 121.45,
                weather: None,
            },
            City {
                name: "Taipei".to_owned(),
                latitude: 25.0330,
                longitude: 121.5654,
                weather: None,
            },
            City {
                name: "Tainan".to_owned(),
                latitude: 23.1725,
                longitude: 120.279,
                weather: None,
            },
            City {
                name: "Eindhoven".to_owned(),
                latitude: 51.44,
                longitude: 5.46,
                weather: None,
            },
            City {
                name: "Tokyo".to_owned(),
                latitude: 35.6895,
                longitude: 139.6917,
                weather: None,
            },
            City {
                name: "Seattle".to_owned(),
                latitude: 47.6062,
                longitude: 122.3321,
                weather: None,
            },
            City {
                name: "Chiang Mai".to_owned(),
                latitude: 18.7061,
                longitude: 98.9817,
                weather: None,
            },
            City {
                name: "Pai".to_owned(),
                latitude: 19.3583,
                longitude: 98.4418,
                weather: None,
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
                    if let Ok(weather) = weather {
                        update_weather(&msg_tx_clone, &city.name, weather).await;
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(WEATHER_POLLING)).await;
            }
        });

        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Trace,
            format!("[{NAME}] init"),
        )
        .await;
    }

    async fn show(&mut self, cmd: &Cmd) {
        match &cmd.reply {
            Reply::Device(_) => {
                for city in &self.weather {
                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!("[{NAME}] {}:", city.name),
                    )
                    .await;

                    let (datetime, temperature, weathercode) = match &city.weather {
                        None => ("n/a".to_owned(), "n/a".to_owned(), "n/a".to_owned()),
                        Some(weather) => (
                            utils::ts_str(utils::datetime_str_to_ts(&weather.time) as u64),
                            weather.temperature.to_string(),
                            utils::weather_code_str(weather.weathercode).to_owned(),
                        ),
                    };

                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!("[{NAME}]     Datetime: {datetime}"),
                    )
                    .await;

                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!("[{NAME}]     Temperature: {temperature}°C"),
                    )
                    .await;

                    log(
                        &self.msg_tx,
                        cmd.reply.clone(),
                        Info,
                        format!("[{NAME}]     Weather: {weathercode}"),
                    )
                    .await;

                    if city.weather.is_none() {
                        continue;
                    }

                    for daily in &city.weather.as_ref().unwrap().daily {
                        log(
                            &self.msg_tx,
                            cmd.reply.clone(),
                            Info,
                            format!(
                                "[{NAME}]     {time}: {temperature_2m_max}°C/{temperature_2m_min}°C, {precipitation_probability_max}%, {weather_code}",
                                time = daily.time,
                                temperature_2m_max = daily.temperature_2m_max,
                                temperature_2m_min = daily.temperature_2m_min,
                                precipitation_probability_max = daily.precipitation_probability_max,
                                weather_code = utils::weather_code_str(daily.weather_code),
                            ),
                        )
                        .await;
                    }
                }
            }
            Reply::Web(sender) => {
                sender
                    .send(serde_json::to_value(self.weather.clone()).unwrap())
                    .await
                    .unwrap();
            }
        }
    }

    async fn update(&mut self, cmd: &Cmd) {
        let msg_tx_clone = self.msg_tx.clone();
        let weather = self.weather.clone();
        let reply_clone = cmd.reply.clone();
        tokio::spawn(async move {
            for city in &weather {
                let weather = utils::weather(city.latitude, city.longitude).await;
                if let Ok(weather) = weather {
                    update_weather(&msg_tx_clone, &city.name, weather).await;
                }
            }
            log(&msg_tx_clone, reply_clone, Info, format!("[{NAME}] update")).await;
        });
    }

    async fn weather(&mut self, cmd: &Cmd) {
        let city_name = cmd.data[0].clone();
        let city = self
            .weather
            .iter_mut()
            .find(|p| p.name == city_name)
            .unwrap_or_else(|| panic!("City not found: {}", city_name));

        city.weather = Some(utils::Weather {
            time: cmd.data[1].clone(),
            temperature: cmd.data[2].parse::<f32>().unwrap(),
            weathercode: cmd.data[3].parse::<u8>().unwrap(),
            daily: vec![],
        });

        msg::weather(&self.msg_tx, self.weather.clone()).await;
    }

    async fn weather_daily(&mut self, cmd: &Cmd) {
        let weather_daily = WeatherDaily {
            time: cmd.data[2].clone(),
            temperature_2m_max: cmd.data[3].parse::<f32>().unwrap(),
            temperature_2m_min: cmd.data[4].parse::<f32>().unwrap(),
            precipitation_probability_max: cmd.data[5].parse::<u8>().unwrap(),
            weather_code: cmd.data[6].parse::<u8>().unwrap(),
        };

        if let Some(city) = self.weather.iter_mut().find(|p| p.name == cmd.data[0]) {
            if let Some(weather) = city.weather.as_mut() {
                // put weather_daily into weather.daily according to cmd.data[1]
                let idx = cmd.data[1].parse::<usize>().unwrap();
                if idx < weather.daily.len() {
                    weather.daily[idx] = weather_daily;
                } else {
                    // put to cmd.data[1] into weather.daily, and fill the gap with None
                    while weather.daily.len() < idx {
                        weather.daily.push(WeatherDaily {
                            time: "n/a".to_owned(),
                            temperature_2m_max: 0.0,
                            temperature_2m_min: 0.0,
                            precipitation_probability_max: 0,
                            weather_code: 0,
                        });
                    }
                    weather.daily.push(weather_daily);
                }
            }
        }

        msg::weather(&self.msg_tx, self.weather.clone()).await;
    }

    async fn help(&self) {
        log(
            &self.msg_tx,
            Reply::Device(cfg::name()),
            Info,
            format!(
                "[{NAME}] {ACT_HELP}, {ACT_INIT}, {ACT_SHOW}, {ACT_WEATHER}, {ACT_UPDATE}",
                NAME = NAME,
                ACT_HELP = msg::ACT_HELP,
                ACT_INIT = msg::ACT_INIT,
                ACT_SHOW = msg::ACT_SHOW,
                ACT_WEATHER = msg::ACT_WEATHER,
                ACT_UPDATE = msg::ACT_UPDATE,
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
                msg::ACT_HELP => self.help().await,
                msg::ACT_INIT => self.init().await,
                msg::ACT_SHOW => self.show(cmd).await,
                msg::ACT_WEATHER => self.weather(cmd).await,
                msg::ACT_WEATHER_DAILY => self.weather_daily(cmd).await,
                msg::ACT_UPDATE => self.update(cmd).await,
                _ => {
                    log(
                        &self.msg_tx,
                        Reply::Device(cfg::name()),
                        Error,
                        format!("[{NAME}] unknown action: {:?}", cmd.action),
                    )
                    .await;
                }
            },
            _ => {
                log(
                    &self.msg_tx,
                    Reply::Device(cfg::name()),
                    Error,
                    format!("[{NAME}] unknown msg: {msg:?}"),
                )
                .await;
            }
        }

        false
    }
}
