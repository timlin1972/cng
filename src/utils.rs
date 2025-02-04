use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{generic_array::GenericArray, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce}; // Or `Aes128Gcm`
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Local};
use rand::RngCore;
use serde::Deserialize;
use sysinfo::System;

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

    Ok(format!("{}:{}", encoded_nonce, encoded_ciphertext))
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
    let response = match reqwest::Client::new()
        .get("https://wttr.in/?format=3")
        .timeout(tokio::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return "n/a".to_owned();
        }
    };

    let response = match response.text().await {
        Ok(response) => response,
        Err(_) => {
            return "n/a".to_owned();
        }
    };

    response.trim().to_owned()
}

#[derive(Deserialize)]
pub struct Weather {
    pub time: String,
    pub temperature: f32,
    pub code: u8,
}

pub async fn weather(latitude: f32, longitude: f32) -> Result<Weather, String> {
    let response = match reqwest::Client::new()
        .get(format!("https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}&current_weather=true"))
        .timeout(tokio::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get weather: {e}"));
        }
    };

    let response = match response.text().await {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get weather: {e}"));
        }
    };

    let weather: serde_json::Value = serde_json::from_str(&response).unwrap();

    let weather = Weather {
        time: weather["current_weather"]["time"]
            .as_str()
            .unwrap()
            .to_owned(),
        temperature: weather["current_weather"]["temperature"].as_f64().unwrap() as f32,
        code: weather["current_weather"]["weathercode"].as_u64().unwrap() as u8,
    };

    Ok(weather)
}

const WEATHER_CODES: [(u8, &str); 28] = [
    (0, "晴天"),
    (1, "多雲時晴"),
    (2, "局部多雲"),
    (3, "陰天"),
    (45, "有霧"),
    (48, "凍霧"),
    (51, "毛毛雨（小雨強度）"),
    (53, "毛毛雨（中雨強度）"),
    (55, "毛毛雨（大雨強度）"),
    (56, "凍雨（小雨強度）"),
    (57, "凍雨（大雨強度）"),
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

pub fn weather_code_str(code: u8) -> &'static str {
    WEATHER_CODES
        .iter()
        .find(|&&(c, _)| c == code)
        .map(|&(_, desc)| desc)
        .unwrap_or("未知天氣")
}

pub async fn get_city_time(city: &str) -> Result<String, String> {
    let response = match reqwest::Client::new()
        .get(format!(
            "http://timeapi.io/api/timezone/zone?timeZone={city}"
        ))
        .timeout(tokio::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get worldtime: {e}"));
        }
    };

    let response = match response.text().await {
        Ok(response) => response,
        Err(e) => {
            return Err(format!("Failed to get worldtime: {e}"));
        }
    };

    let worldtime: serde_json::Value = serde_json::from_str(&response).unwrap();

    Ok(worldtime["currentLocalTime"].as_str().unwrap().to_owned())
}

// convert YYYY-MM-DDTHH:MM:SS.ffffff to YYYY/MM/DD HH:MM:SS
pub fn convert_datetime(datetime: &str) -> Result<String, String> {
    let datetime = datetime.replace("T", " ").replace("Z", "");
    let datetime = datetime.split('.').next().unwrap();
    Ok(datetime.to_owned())
}
