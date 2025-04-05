use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use log::Level::{Error, Info};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc::Sender;
use unicode_width::UnicodeWidthChar;

use crate::msg::{log, City, Data, DevInfo, Msg, Reply, Worldtime};
use crate::panels::panels_main::{self, PanelInfo, Popup};
use crate::utils;
use crate::{cfg, msg};
use crate::{error, info, init, unknown};

pub const NAME: &str = "Infos";

const POPUP_ALL: &str = "All";
const POPUP_HELP: &str = "Help";
const DEVICES_POLLING: u64 = 60;
const TABS: usize = 7;

fn format_date(input: &str) -> String {
    let date = NaiveDate::parse_from_str(input, "%Y-%m-%d").expect("無法解析日期");
    format!("{} {}", date.format("%m/%d"), date.weekday())
}
#[derive(Debug)]
pub struct Panel {
    panel_info: PanelInfo,
    devices: Vec<DevInfo>,
    tab_index: usize,
    weather: Vec<City>,
    worldtime: Vec<Worldtime>,
    stocks: Vec<utils::Stock>,
}

impl Panel {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        let help_text: Vec<String> = vec![
            "Commands:".to_owned(),
            "h    - Help".to_owned(),
            "⭠ / ⭢  - Change tab".to_owned(),
        ];

        let panel_info = PanelInfo::new(
            NAME,
            vec![
                Popup {
                    name: POPUP_HELP.to_owned(),
                    x: 50,
                    y: 30,
                    output: help_text,
                    cursor_x: None,
                    cursor_y: None,
                },
                Popup {
                    name: POPUP_ALL.to_owned(),
                    x: 100,
                    y: 80,
                    output: vec![],
                    cursor_x: None,
                    cursor_y: None,
                },
            ],
            msg_tx,
        );

        Self {
            panel_info,
            devices: vec![],
            tab_index: 0,
            weather: vec![],
            worldtime: vec![],
            stocks: vec![],
        }
    }

    pub fn tab_refresh(&mut self) {
        self.panel_info.output.clear();

        match self.tab_index {
            0 => {
                self.panel_info.output.push(format!(
                    "{:<12} {:<7} {:<10} {:18} {:14} {:9} {:10} {:<7} {:<11} {:<10}",
                    "Name",
                    "Onboard",
                    "Version",
                    "OS",
                    "CPU Arch/Usage",
                    "Mem Usage",
                    "Disk Usage",
                    "Temp",
                    "Last update",
                    "Countdown"
                ));

                for device in self.devices.iter() {
                    // onboard
                    let onboard = if let Some(t) = device.onboard {
                        if t {
                            "On"
                        } else {
                            "Off"
                        }
                    } else {
                        "n/a"
                    };

                    // version
                    let version = if let Some(t) = &device.version {
                        t.clone()
                    } else {
                        "n/a".to_owned()
                    };

                    let os = if let Some(t) = &device.os {
                        t.clone()
                    } else {
                        "n/a".to_owned()
                    };

                    let cpu = if let (Some(t), Some(u)) = (&device.cpu_arch, &device.cpu_usage) {
                        format!("{}/{:.1}%", t, u)
                    } else {
                        "n/a".to_owned()
                    };

                    // memory
                    let memory_usage = if let Some(t) = device.memory_usage {
                        format!("{:.1}%", t)
                    } else {
                        "n/a".to_owned()
                    };

                    // disk
                    let disk_usage = if let Some(t) = device.disk_usage {
                        format!("{:.1}%", t)
                    } else {
                        "n/a".to_owned()
                    };

                    // temperature
                    let temperature = if let Some(t) = device.temperature {
                        format!("{:.1}°C", t)
                    } else {
                        "n/a".to_owned()
                    };

                    // countdown
                    let countdown = match utils::ts().cmp(&device.ts) {
                        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => 10.to_string(),
                        std::cmp::Ordering::Greater => {
                            if (utils::ts() - device.ts) / 60 >= 10 {
                                "failed".to_owned()
                            } else {
                                (10 - (utils::ts() - device.ts) / 60).to_string()
                            }
                        }
                    };

                    self.panel_info.output.push(format!(
                        "{:<12} {onboard:<7} {version:<10} {os:<18} {cpu:14} {memory_usage:9} {disk_usage:10} {temperature:<7} {:<11} {countdown:<10}",
                        device.name,
                        utils::ts_str(device.ts),
                    ));
                }
            }
            1 => {
                self.panel_info.output.push(format!(
                    "{:<12} {:<7} {:13} {:13} {:16}",
                    "Name", "Onboard", "App uptime", "Host uptime", "Tailscale IP"
                ));
                for device in self.devices.iter() {
                    // onboard
                    let onboard = if let Some(t) = device.onboard {
                        if t {
                            "On"
                        } else {
                            "Off"
                        }
                    } else {
                        "n/a"
                    };

                    // app uptime
                    let app_uptime = if let Some(t) = device.app_uptime {
                        utils::uptime_str(t)
                    } else {
                        "n/a".to_owned()
                    };

                    // host uptime
                    let host_uptime = if let Some(t) = device.host_uptime {
                        utils::uptime_str(t)
                    } else {
                        "n/a".to_owned()
                    };

                    self.panel_info.output.push(format!(
                        "{:<12} {onboard:<7} {app_uptime:13} {host_uptime:13} {:16}",
                        device.name,
                        device.tailscale_ip.clone().unwrap_or("n/a".to_owned())
                    ));
                }
            }
            2 => {
                self.panel_info.output.push(format!(
                    "{:<12} {:<7} {:<27} {:64}",
                    "Name", "Onboard", "Last seen", "Weather"
                ));
                for device in self.devices.iter() {
                    // onboard
                    let onboard = if let Some(t) = device.onboard {
                        if t {
                            "On"
                        } else {
                            "Off"
                        }
                    } else {
                        "n/a"
                    };

                    // weather
                    let weather = if let Some(t) = &device.weather {
                        t.clone()
                    } else {
                        "n/a".to_owned()
                    };

                    // last_seen
                    let last_seen = if let Some(t) = device.last_seen {
                        utils::ts_str_full(t)
                    } else {
                        "n/a".to_owned()
                    };

                    self.panel_info.output.push(format!(
                        "{:<12} {onboard:<7} {last_seen:<27} {weather:64}",
                        device.name,
                        onboard = onboard,
                        last_seen = last_seen,
                        weather = weather,
                    ));
                }
            }
            3 => {
                self.panel_info.output.push(format!(
                    "{:<12} {:<11} {:7} {:20}",
                    "City", "Update", "Temp", "Weather"
                ));
                for city in &self.weather {
                    let (update, temperature, weather) = match &city.weather {
                        Some(weather) => (
                            utils::ts_str(utils::datetime_str_to_ts(&weather.time) as u64),
                            format!("{:.1}°C", weather.temperature),
                            utils::weather_code_str(weather.weathercode).to_owned(),
                        ),
                        None => ("n/a".to_owned(), "n/a".to_owned(), "n/a".to_owned()),
                    };

                    self.panel_info.output.push(format!(
                        "{:<12} {update:<11} {temperature:7} {weather:20}",
                        city.name
                    ));
                }
            }
            4 => {
                if self.weather.is_empty() {
                    return;
                }
                if self.weather[0].weather.is_none() {
                    return;
                }

                let weather = self.weather[0].weather.as_ref().unwrap();
                let mut title = String::new();
                title.push_str(&format!("{:<12} ", "City"));
                for (idx, daily) in weather.daily.iter().enumerate() {
                    if idx == 0 {
                        continue;
                    }
                    title.push_str(&format!("{:<27} ", format_date(&daily.time)));
                }
                self.panel_info.output.push(title);

                for city in &self.weather {
                    if let Some(weather) = &city.weather {
                        let mut info = String::new();
                        for (idx, daily) in weather.daily.iter().enumerate() {
                            if idx == 0 {
                                continue;
                            }
                            let (
                                temperature,
                                precipitation_probability_max,
                                weather_emoji,
                                weather,
                            ) = (
                                format!(
                                    "{:.0}/{:.0}",
                                    daily.temperature_2m_max, daily.temperature_2m_min
                                ),
                                format!("{}%", daily.precipitation_probability_max),
                                utils::weather_code_emoji(daily.weather_code).to_owned(),
                                utils::weather_code_str(daily.weather_code).to_owned(),
                            );
                            info.push_str(&format!(
                                "{weather_emoji} {precipitation_probability_max:3} {temperature:7} "
                            ));
                            info.push_str(&weather);
                            info.push_str(" ".repeat(13 - weather.len() * 2 / 3).as_str());
                        }

                        self.panel_info
                            .output
                            .push(format!("{:<12} {info}", city.name));
                    }
                }
            }
            5 => {
                self.panel_info
                    .output
                    .push(format!("{:<12} {:<11}", "City", "Datetime"));
                for city in &self.worldtime {
                    self.panel_info
                        .output
                        .push(format!("{:<12} {:<11}", city.name, city.datetime));
                }
            }
            6 => {
                self.panel_info.output.push(format!(
                    "{:<4} {:<8} {:<18} {:<7} {:<12}  {:<7}  {:<7}",
                    "Code", "Name", "Update", "Last", "Change", "Low", "High"
                ));
                for stock in &self.stocks {
                    let mut info = String::new();

                    let last_price = stock.last_price.parse::<f32>().unwrap_or(0.0);
                    let high_price = stock.high_price.parse::<f32>().unwrap_or(0.0);
                    let low_price = stock.low_price.parse::<f32>().unwrap_or(0.0);
                    let prev_close = stock.prev_close.parse::<f32>().unwrap_or(0.0);
                    let change = last_price - prev_close;
                    let change_percent = if prev_close != 0.0 {
                        change / prev_close * 100.0
                    } else {
                        0.0
                    };

                    info.push_str(&format!("{:<4} {} ", stock.code, stock.name));
                    let width: usize = stock.name.chars().map(|c| c.width().unwrap_or(0)).sum();
                    info.push_str(" ".repeat(8 - width).as_str());
                    info.push_str(&format!("{:<18} {last_price:<7.2} {change:>5.2} {change_percent:>5.2}%  {low_price:<7.2}  {high_price:<7.2}", stock.datetime));

                    self.panel_info.output.push(info.to_string());
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl panels_main::Panel for Panel {
    fn get_panel_info(&self) -> &PanelInfo {
        &self.panel_info
    }

    fn title(&self) -> String {
        format!("{} - {}/{TABS}", self.panel_info.name, self.tab_index + 1)
    }

    async fn init(&mut self) {
        init!(&self.panel_info.msg_tx, NAME);

        let msg_tx_clone = self.panel_info.msg_tx.clone();
        tokio::spawn(async move {
            loop {
                msg::device_countdown(&msg_tx_clone).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(DEVICES_POLLING)).await;
            }
        });
    }

    async fn msg(&mut self, msg: &Msg) {
        match &msg.data {
            Data::Devices(devices) => {
                self.devices = devices.clone();
                self.tab_refresh();
            }
            Data::DeviceCountdown => {
                self.tab_refresh();
            }
            Data::Weather(weather) => {
                self.weather = weather.clone();
                self.tab_refresh();
            }
            Data::Worldtime(worldtime) => {
                self.worldtime = worldtime.clone();
                self.tab_refresh();
            }
            Data::Stocks(stocks) => {
                self.stocks = stocks.clone();
                self.tab_refresh();
            }
            _ => {
                unknown!(&self.panel_info.msg_tx, NAME, msg);
            }
        }
    }

    async fn key(&mut self, key: KeyEvent) -> bool {
        match self.panel_info.active_popup_name.is_some() {
            true => self.panel_info.active_popup_name = None,
            false => match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('h') => {
                    self.panel_info.active_popup_name = Some(POPUP_HELP.to_owned());
                }
                KeyCode::Right => {
                    self.tab_index = (self.tab_index + 1) % TABS;
                    self.tab_refresh();
                }
                KeyCode::Left => {
                    self.tab_index = (self.tab_index + TABS - 1) % TABS;
                    self.tab_refresh();
                }
                KeyCode::Char('a') => {
                    self.panel_info.active_popup_name = Some(POPUP_ALL.to_owned());
                    let active_popup = self
                        .panel_info
                        .popup
                        .iter_mut()
                        .find(|p| p.name == POPUP_ALL)
                        .unwrap();

                    active_popup.output = self.panel_info.output.clone();
                }
                _ => {}
            },
        }

        false
    }
}
