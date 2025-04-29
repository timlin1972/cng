use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{generic_array::GenericArray, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce}; // Or `Aes128Gcm`
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use sysinfo::System;

pub fn format_number(num: u64) -> String {
    if num >= 1_000_000 {
        format!("{:.1}M", num as f64 / 1_000_000.0)
    } else if num >= 1_000 {
        format!("{:.1}k", num as f64 / 1_000.0)
    } else {
        num.to_string()
    }
}

fn format_speed(num: f64) -> String {
    if num >= 1_000_000_000.0 {
        format!("{:.1}GB/s", num / 1_000_000_000.0)
    } else if num >= 1_000_000.0 {
        format!("{:.1}MB/s", num / 1_000_000.0)
    } else if num >= 1_000.0 {
        format!("{:.1}KB/s", num / 1_000.0)
    } else {
        format!("{:.1}B/s", num)
    }
}

pub fn transmit_str(transmit_size: u64, escaped_time: u64) -> String {
    let escaped_time = if escaped_time == 0 { 1 } else { escaped_time };
    let speed = transmit_size as f64 / escaped_time as f64;

    format!(
        "{} ({}, {escaped_time}s)",
        format_speed(speed),
        format_number(transmit_size)
    )
}

pub fn ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn ts_str(ts: u64) -> String {
    let datetime_local: DateTime<Local> = DateTime::from_timestamp(ts as i64, 0)
        .unwrap()
        .with_timezone(&Local);

    datetime_local.format("%H:%M:%S").to_string()
}

pub fn ts_str_full(ts: u64) -> String {
    let datetime_local: DateTime<Local> = DateTime::from_timestamp(ts as i64, 0)
        .unwrap()
        .with_timezone(&Local);

    datetime_local.format("%Y-%m-%d %H:%M:%S %:z").to_string()
}

pub fn ts_str_short(ts: u64) -> String {
    let datetime_local: DateTime<Local> = DateTime::from_timestamp(ts as i64, 0)
        .unwrap()
        .with_timezone(&Local);

    datetime_local.format("%m-%d %H:%M:%S").to_string()
}

pub fn uptime() -> u64 {
    System::uptime()
}

pub fn uptime_str(uptime: u64) -> String {
    let mut uptime = uptime;
    let days = uptime / 86400;
    uptime -= days * 86400;
    let hours = uptime / 3600;
    uptime -= hours * 3600;
    let minutes = uptime / 60;
    let seconds = uptime % 60;

    format!("{days}d {hours:02}:{minutes:02}:{seconds:02}")
}

pub fn encrypt(key: &str, plaintext: &str) -> Result<String, String> {
    if key.len() != 32 {
        return Err("Key length must be 32.".to_owned());
    }

    let key = GenericArray::from_slice(key.as_bytes());
    let cipher = Aes256Gcm::new(key);

    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let nonce = Nonce::from_slice(&nonce); // 96-bits; unique per message

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| format!("Encryption error: {e:?}"))?;

    let encoded_ciphertext = general_purpose::STANDARD.encode(&ciphertext);
    let encoded_nonce = general_purpose::STANDARD.encode(nonce);

    Ok(format!("{encoded_nonce}:{encoded_ciphertext}"))
}

pub fn decrypt(key: &str, enc_str: &str) -> Result<String, String> {
    if key.len() != 32 {
        return Err("Key length must be 32.".to_owned());
    }

    let key = GenericArray::from_slice(key.as_bytes());

    let cipher = Aes256Gcm::new(key);

    let parts: Vec<&str> = enc_str.split(':').collect();
    let encoded_nonce = parts.first().unwrap();
    let encoded_ciphertext = parts
        .get(1)
        .ok_or_else(|| format!("Err: Failed to decrypt: {enc_str}"))?;

    let nonce = general_purpose::STANDARD
        .decode(encoded_nonce)
        .map_err(|e| format!("Err: Failed to decrypt: {enc_str}, err: {e}"))?;
    let ciphertext = general_purpose::STANDARD
        .decode(encoded_ciphertext)
        .expect("Decoding ciphertext failed");

    let decrypted_plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|e| format!("Err: Failed to decrypt: {enc_str}, err: {e}"))?;
    String::from_utf8(decrypted_plaintext).map_err(|_| "Decryption failed".to_owned())
}
pub async fn device_weather() -> String {
    let client = reqwest::Client::new();
    let response = client
        .get("https://wttr.in/?format=3")
        .timeout(tokio::time::Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(resp) => match resp.text().await {
            Ok(text) => text.trim().to_owned(),
            Err(_) => "n/a".to_owned(),
        },
        Err(_) => "n/a".to_owned(),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeatherDaily {
    pub time: String,
    pub temperature_2m_max: f32,
    pub temperature_2m_min: f32,
    pub precipitation_probability_max: u8,
    pub weather_code: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Weather {
    pub time: String,
    pub temperature: f32,
    pub weathercode: u8,
    pub daily: Vec<WeatherDaily>,
}

pub async fn weather(latitude: f32, longitude: f32) -> Result<Weather, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}&daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max,weather_code&current_weather=true"
    );

    let response = client
        .get(url)
        .timeout(tokio::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Failed to get weather: {e}"))?;

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to get weather: {e}"))?;

    let weather_data: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse weather data: {e}"))?;

    let current = &weather_data["current_weather"];
    let time = current["time"]
        .as_str()
        .ok_or("Missing current_weather.time")?
        .to_string();
    let temperature = current["temperature"]
        .as_f64()
        .ok_or("Missing or invalid current_weather.temperature")? as f32;
    let weathercode = current["weathercode"]
        .as_u64()
        .ok_or("Missing or invalid current_weather.weathercode")? as u8;

    let (max_temps, min_temps, precip_probs, weather_codes, dates) = (
        weather_data["daily"]["temperature_2m_max"]
            .as_array()
            .ok_or("Missing daily.temperature_2m_max")?,
        weather_data["daily"]["temperature_2m_min"]
            .as_array()
            .ok_or("Missing daily.temperature_2m_min")?,
        weather_data["daily"]["precipitation_probability_max"]
            .as_array()
            .ok_or("Missing daily.precipitation_probability_max")?,
        weather_data["daily"]["weather_code"]
            .as_array()
            .ok_or("Missing daily.weather_code")?,
        weather_data["daily"]["time"]
            .as_array()
            .ok_or("Missing daily.time")?,
    );

    let len = max_temps.len();
    if min_temps.len() != len
        || precip_probs.len() != len
        || weather_codes.len() != len
        || dates.len() != len
    {
        return Err("Mismatch in forecast array lengths".to_string());
    }

    let mut daily_forecast = Vec::new();
    for i in 0..len {
        let daily = WeatherDaily {
            time: dates[i]
                .as_str()
                .ok_or("Invalid daily.time")?
                .to_string(),
            temperature_2m_max: max_temps[i]
                .as_f64()
                .ok_or("Invalid temperature_2m_max")? as f32,
            temperature_2m_min: min_temps[i]
                .as_f64()
                .ok_or("Invalid temperature_2m_min")? as f32,
            precipitation_probability_max: precip_probs[i]
                .as_f64()
                .ok_or("Invalid precipitation_probability_max")? as u8,
            weather_code: weather_codes[i]
                .as_u64()
                .ok_or("Invalid weather_code")? as u8,
        };
        daily_forecast.push(daily);
    }

    Ok(Weather {
        time,
        temperature,
        weathercode,
        daily: daily_forecast,
    })
}

const WEATHER_CODES: [(u8, &str); 28] = [
    (0, "晴天"),
    (1, "多雲時晴"),
    (2, "局部多雲"),
    (3, "陰天"),
    (45, "有霧"),
    (48, "凍霧"),
    (51, "毛毛雨（小）"),
    (53, "毛毛雨（中）"),
    (55, "毛毛雨（大）"),
    (56, "凍雨（小）"),
    (57, "凍雨（大）"),
    (61, "小雨"),
    (63, "中雨"),
    (65, "大雨"),
    (66, "凍雨（小雨）"),
    (67, "凍雨（大雨）"),
    (71, "小雪"),
    (73, "中雪"),
    (75, "大雪"),
    (77, "雪粒"),
    (80, "小陣雨"),
    (81, "中陣雨"),
    (82, "強陣雨"),
    (85, "小陣雪"),
    (86, "大陣雪"),
    (95, "雷雨"),
    (96, "雷雨夾小冰雹"),
    (99, "雷雨夾大冰雹"),
];

const WEATHER_CODES_EMOJI: [(u8, &str); 28] = [
    (0, "☀️"),
    (1, "🌤️"),
    (2, "⛅"),
    (3, "☁️"),
    (45, "🌫️"),
    (48, "❄️"),
    (51, "🌧️"),
    (53, "🌧️"),
    (55, "🌧️"),
    (56, "❄️"),
    (57, "❄️"),
    (61, "🌧️"),
    (63, "🌧️"),
    (65, "🌧️"),
    (66, "❄️"),
    (67, "❄️"),
    (71, "🌨️"),
    (73, "🌨️"),
    (75, "🌨️"),
    (77, "❄️"),
    (80, "🌦️"),
    (81, "🌦️"),
    (82, "🌦️"),
    (85, "🌨️"),
    (86, "🌨️"),
    (95, "⛈️"),
    (96, "⛈️"),
    (99, "⛈️"),
];

pub fn weather_code_str(code: u8) -> &'static str {
    WEATHER_CODES
        .iter()
        .find(|&&(c, _)| c == code)
        .map(|&(_, desc)| desc)
        .unwrap_or("未知天氣")
}

pub fn weather_code_emoji(code: u8) -> &'static str {
    WEATHER_CODES_EMOJI
        .iter()
        .find(|&&(c, _)| c == code)
        .map(|&(_, desc)| desc)
        .unwrap_or("未知天氣")
}
pub async fn get_city_time(city: &str) -> Result<String, String> {
    let url = reqwest::Url::parse_with_params(
        "http://timeapi.io/api/timezone/zone",
        &[("timeZone", city)],
    ).map_err(|e| format!("Invalid URL: {e}"))?;

    let response = reqwest::Client::new()
        .get(url)
        .timeout(tokio::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {e}"))?;

    let worldtime: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse worldtime JSON: {e}"))?;

    let current_time = worldtime["currentLocalTime"]
        .as_str()
        .ok_or_else(|| "Missing 'currentLocalTime' field".to_string())?
        .to_owned();

    Ok(current_time)
}

// convert YYYY-MM-DDTHH:MM:SS.ffffff to YYYY/MM/DD HH:MM:SS
pub fn convert_datetime(datetime: &str) -> Result<String, String> {
    let datetime = datetime.replace("T", " ").replace("Z", "");
    let datetime = datetime.split('.').next().unwrap();
    Ok(datetime.to_owned())
}

use chrono::NaiveDateTime;

pub fn datetime_str_to_ts(datetime_str: &str) -> i64 {
    let naive_datetime = NaiveDateTime::parse_from_str(datetime_str, "%Y-%m-%dT%H:%M")
        .expect("解析日期時間字串失敗");
    naive_datetime.and_utc().timestamp()
}

use sysinfo::Networks;

const TAILSCALE_INTERFACE: &str = "tailscale";
const TAILSCALE_INTERFACE_MAC: &str = "utun";

pub fn get_tailscale_ip() -> String {
    let networks = Networks::new_with_refreshed_list();
    for (interface_name, network) in &networks {
        if interface_name.starts_with(TAILSCALE_INTERFACE) {
            for ipnetwork in network.ip_networks().iter() {
                // if ipv4
                if ipnetwork.addr.is_ipv4() {
                    return ipnetwork.addr.to_string();
                }
            }
        }
        if interface_name.starts_with(TAILSCALE_INTERFACE_MAC) {
            for ipnetwork in network.ip_networks().iter() {
                // if ipv4
                if let std::net::IpAddr::V4(ip) = ipnetwork.addr {
                    // if the first 1 byte is 100, it's a tailscale ip
                    if ip.octets()[0] == 100 {
                        return ipnetwork.addr.to_string();
                    }
                }
            }
        }
    }
    "n/a".to_string()
}

use std::fs::File;
use std::io::{BufReader, Read};

pub fn calculate_md5(path: &str) -> std::io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    let digest = md5::compute(buffer);
    Ok(format!("{:x}", digest))
}

#[derive(Debug, Clone)]
pub struct Stock {
    pub code: String,
    pub name: String,
    pub last_price: String,
    pub high_price: String,
    pub low_price: String,
    pub prev_close: String,
    pub datetime: String,
}

impl Stock {
    pub fn new(code: String) -> Self {
        Self {
            code,
            name: "n/a".to_owned(),
            last_price: "n/a".to_owned(),
            high_price: "n/a".to_owned(),
            low_price: "n/a".to_owned(),
            prev_close: "n/a".to_owned(),
            datetime: "n/a".to_owned(),
        }
    }
}

pub async fn get_stock_info(code: &str) -> Result<Stock, String> {
    let response = match reqwest::Client::new()
        .get(format!(
            "https://mis.twse.com.tw/stock/api/getStockInfo.jsp?ex_ch=tse_{code}.tw"
        ))
        .timeout(tokio::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get stock_price: {e}"));
        }
    };

    let response = match response.text().await {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get stock_price: {e}"));
        }
    };

    let response: serde_json::Value = match serde_json::from_str(&response) {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get stock_price: {e}"));
        }
    };

    let stock = response["msgArray"].get(0);
    if stock.is_none() {
        return Err("無此股票".to_owned());
    }
    let stock = stock.unwrap();

    let name = stock["n"].as_str().unwrap_or("n/a"); //  股票名稱
    let last_price = stock["z"].as_str().unwrap_or("n/a"); //  最新成交價
                                                           // let open_price = stock["o"].as_str().unwrap_or("n/a");   //  開盤價
    let high_price = stock["h"].as_str().unwrap_or("n/a"); //  最高價
    let low_price = stock["l"].as_str().unwrap_or("n/a"); //  最低價
    let prev_close = stock["y"].as_str().unwrap_or("n/a"); //  昨日收盤價
                                                           // let volume = stock["v"].as_str().unwrap_or("n/a");       //  成交量
                                                           // let time = stock["t"].as_str().unwrap_or("n/a");         //  最後成交時間

    let datetime = format!(
        "{} {}",
        stock["d"].as_str().unwrap_or("n/a"),
        stock["t"].as_str().unwrap_or("n/a")
    );

    Ok(Stock {
        code: code.to_owned(),
        name: name.to_owned(),
        last_price: last_price.to_owned(),
        high_price: high_price.to_owned(),
        low_price: low_price.to_owned(),
        prev_close: prev_close.to_owned(),
        datetime: datetime.to_owned(),
    })
}
