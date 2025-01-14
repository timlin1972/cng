use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{generic_array::GenericArray, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce}; // Or `Aes128Gcm`
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Local};
use rand::RngCore;
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

    format!("{days}d {hours}:{minutes:02}:{seconds:02}")
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
