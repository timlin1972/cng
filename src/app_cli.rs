use std::io::Write;

use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{
    command,
    msg::{self, Msg},
    panels::panels_main,
    plugins::plugins_main,
    utils, KEY_SIZE,
};

fn prompt() -> Result<(), String> {
    let ts_str = utils::ts_str(utils::ts());

    print!("{ts_str} > ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    Ok(())
}

pub struct App {
    plugins: plugins_main::Plugins,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    key_rx: Receiver<String>,
}

impl App {
    pub fn new(msg_tx: Sender<Msg>, msg_rx: Receiver<Msg>) -> Self {
        // read key
        let (key_tx, key_rx) = mpsc::channel(KEY_SIZE);
        tokio::spawn(async move {
            let stdin = io::stdin(); // 標準輸入
            let reader = BufReader::new(stdin); // 使用緩衝讀取
            let mut lines = reader.lines();

            prompt().unwrap();
            while let Ok(Some(line)) = lines.next_line().await {
                if key_tx.send(line).await.is_err() {
                    // 如果接收端已關閉，停止 task
                    println!("Receiver dropped, stopping input task.");
                    break;
                }
                // waiting for 1 second to avoid the prompt being mixed with the output
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                prompt().unwrap();
            }
        });

        Self {
            plugins: plugins_main::Plugins::new(msg_tx.clone()),
            msg_tx,
            msg_rx,
            key_rx,
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.plugins.init().await;

        loop {
            tokio::select! {
                Some(msg) = self.msg_rx.recv() => {
                    if msg.plugin == panels_main::NAME {
                        match &msg.data {
                            msg::Data::Devices(_devices) => {
                                // do nothing
                            }
                            msg::Data::Worldtime(_worldtime) => {
                                // do nothing
                            }
                            msg::Data::Weather(_weather) => {
                                // do nothing
                            }
                            _ => {
                                println!("Err: panels_main, msg: {:?}", msg);
                            }
                        }
                    }
                    else if self.plugins.msg(&msg).await { return Ok(()) }
                }
                Some(line) = self.key_rx.recv() => {
                    // if line is empty, skip
                    if line.trim().is_empty() {
                        continue;
                    }
                    if command::run(&self.msg_tx, &line).await {
                        return Ok(());
                    }
                }
            }
        }
    }
}
