use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

pub const DEF_NAME: &str = "cng_default";
const CFG_FILE: &str = "./cfg.json";

#[derive(Serialize, Deserialize)]
pub struct Cfg {
    pub name: String,
}

pub fn init() {
    let path = Path::new(CFG_FILE);

    if !path.exists() {
        let cfg = Cfg {
            name: DEF_NAME.to_owned(),
        };

        let file_content = serde_json::to_string_pretty(&cfg).unwrap();

        // lock
        let mut file = File::create(CFG_FILE).unwrap();
        file.lock_exclusive().unwrap();

        file.write_all(file_content.as_bytes()).unwrap();

        // unlock
        file.unlock().unwrap();
    }
}

pub fn get_cfg() -> Cfg {
    // lock
    let file = File::open(CFG_FILE).unwrap();
    file.lock_shared().unwrap();

    let file_content = fs::read_to_string(CFG_FILE).unwrap();
    let cfg: Cfg = serde_json::from_str(&file_content).unwrap();

    // unlock
    file.unlock().unwrap();

    cfg
}

pub fn get_name() -> String {
    get_cfg().name
}
