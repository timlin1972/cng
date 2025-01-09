use std::fs::OpenOptions;
use std::io::Write;

const DEF_LOG_FILE: &str = "log.txt";

// log message to file
pub fn log_to_file(msg: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(DEF_LOG_FILE)
        .unwrap();

    writeln!(file, "{}", msg).unwrap();
}
