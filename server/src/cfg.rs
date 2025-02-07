use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub const DEF_NAME: &str = "cng_default";
const CFG_FILE: &str = "./cfg.json";
const KEY: &str = "an example very very secret key."; // length is fixed
const SHELL: &str = "sh";
pub const MODE_CLI: &str = "cli";
pub const MODE_GUI: &str = "gui";
const TRACE: u8 = 1;

static INSTANCE: Lazy<Mutex<Cfg>> = Lazy::new(|| Mutex::new(Cfg::new()));

fn default_name() -> String {
    DEF_NAME.to_string()
}

fn default_key() -> String {
    KEY.to_string()
}

fn default_sh() -> String {
    SHELL.to_string()
}

fn default_mode() -> String {
    MODE_CLI.to_string()
}

fn default_trace() -> u8 {
    TRACE
}

#[derive(Serialize, Deserialize)]
pub struct Cfg {
    #[serde(default = "default_name")]
    name: String,
    #[serde(default = "default_key")]
    key: String,
    #[serde(default = "default_sh")]
    shell: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_trace")]
    trace: u8,
    db: String,
}

impl Cfg {
    pub fn new() -> Self {
        let path = Path::new(CFG_FILE);

        let cfg = if !path.exists() {
            Cfg {
                name: DEF_NAME.to_owned(),
                key: KEY.to_owned(),
                shell: SHELL.to_owned(),
                mode: MODE_CLI.to_owned(),
                trace: TRACE,
                db: "mongodb://localhost:27017".to_owned(),
            }
        } else {
            let file_content = fs::read_to_string(CFG_FILE).unwrap();
            serde_json::from_str(&file_content).unwrap()
        };

        let file_content = serde_json::to_string_pretty(&cfg).unwrap();
        let mut file = File::create(CFG_FILE).unwrap();
        file.write_all(file_content.as_bytes()).unwrap();

        cfg
    }

    pub fn get_instance() -> std::sync::MutexGuard<'static, Cfg> {
        INSTANCE.lock().unwrap()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn key(&self) -> &str {
        &self.key
    }

    fn shell(&self) -> &str {
        &self.shell
    }

    fn mode(&self) -> &str {
        &self.mode
    }

    fn trace(&self) -> u8 {
        self.trace
    }
}

pub fn name() -> String {
    let cfg = Cfg::get_instance();
    cfg.name().to_owned()
}

pub fn key() -> String {
    let cfg = Cfg::get_instance();
    cfg.key().to_owned()
}

pub fn shell() -> String {
    let cfg = Cfg::get_instance();
    cfg.shell().to_owned()
}

pub fn mode() -> String {
    let cfg = Cfg::get_instance();
    cfg.mode().to_owned()
}

pub fn trace() -> u8 {
    let cfg = Cfg::get_instance();
    cfg.trace()
}

pub fn db() -> String {
    let cfg = Cfg::get_instance();
    cfg.db.to_owned()
}
