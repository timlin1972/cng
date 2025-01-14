use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const DEF_NAME: &str = "cng_default";
const CFG_FILE: &str = "./cfg.json";
const KEY: &str = "an example very very secret key."; // length is fixed

#[derive(Serialize, Deserialize)]
pub struct Cfg {
    pub name: String,
    pub key: String,
}

pub fn init() {
    let path = Path::new(CFG_FILE);

    if !path.exists() {
        let cfg = Cfg {
            name: DEF_NAME.to_owned(),
            key: KEY.to_owned(),
        };

        let file_content = serde_json::to_string_pretty(&cfg).unwrap();

        // lock
        let mut file = File::create(CFG_FILE).unwrap();
        fs2::FileExt::lock_exclusive(&file).unwrap();

        file.write_all(file_content.as_bytes()).unwrap();

        // unlock
        fs2::FileExt::unlock(&file).unwrap();
    }
}

pub fn get_cfg() -> Cfg {
    // lock
    let file = File::open(CFG_FILE).unwrap();
    fs2::FileExt::lock_shared(&file).unwrap();

    let file_content = fs::read_to_string(CFG_FILE).unwrap();
    let cfg: Cfg = serde_json::from_str(&file_content).unwrap();

    // unlock
    fs2::FileExt::unlock(&file).unwrap();

    cfg
}

pub fn get_name() -> String {
    get_cfg().name
}

pub fn get_key() -> String {
    get_cfg().key
}
