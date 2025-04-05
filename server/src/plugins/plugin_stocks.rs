use async_trait::async_trait;
use log::Level::{Error, Info};
use tokio::sync::mpsc::Sender;
use unicode_width::UnicodeWidthChar;

use crate::msg::{self, log, Cmd, Data, Msg, Reply};
use crate::plugins::plugins_main;
use crate::{
    cfg,
    utils::{self, Stock},
};
use crate::{error, info, init, reply_me, unknown};

pub const NAME: &str = "stocks";
const STOCK_POLLING: u64 = 60;

#[derive(Debug)]
pub struct Plugin {
    name: String,
    msg_tx: Sender<Msg>,
    stocks: Vec<Stock>,
}

impl Plugin {
    pub fn new(msg_tx: Sender<Msg>) -> Self {
        Self {
            name: NAME.to_owned(),
            msg_tx,
            stocks: vec![
                Stock::new("2317".to_owned()), // 鴻海
                Stock::new("6782".to_owned()), // 視陽
                Stock::new("2412".to_owned()), // 中華電
                Stock::new("2330".to_owned()), // 台積電
            ],
        }
    }

    async fn init(&mut self) {
        let msg_tx_clone = self.msg_tx.clone();
        let stocks = self.stocks.clone();
        tokio::spawn(async move {
            loop {
                for stock in &stocks {
                    let stock_info = utils::get_stock_info(&stock.code).await;
                    if let Ok(stock_info) = stock_info {
                        msg::cmd(
                            &msg_tx_clone,
                            reply_me!(),
                            NAME.to_owned(),
                            msg::ACT_STOCK.to_owned(),
                            vec![
                                stock_info.code.clone(),
                                stock_info.name.clone(),
                                stock_info.last_price.clone(),
                                stock_info.high_price.clone(),
                                stock_info.low_price.clone(),
                                stock_info.prev_close.clone(),
                                stock_info.datetime.clone(),
                            ],
                        )
                        .await;
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(STOCK_POLLING)).await;
            }
        });

        init!(&self.msg_tx, NAME);
    }

    async fn update(&mut self, cmd: &Cmd) {
        let msg_tx_clone = self.msg_tx.clone();
        let stocks = self.stocks.clone();
        let reply_clone = cmd.reply.clone();
        tokio::spawn(async move {
            for stock in &stocks {
                let stock_info = utils::get_stock_info(&stock.code).await;

                info!(&msg_tx_clone, format!("[{NAME}] {} updated.", stock.code));

                if let Ok(stock_info) = stock_info {
                    msg::cmd(
                        &msg_tx_clone,
                        reply_me!(),
                        NAME.to_owned(),
                        msg::ACT_STOCK.to_owned(),
                        vec![
                            stock_info.code.clone(),
                            stock_info.name.clone(),
                            stock_info.last_price.clone(),
                            stock_info.high_price.clone(),
                            stock_info.low_price.clone(),
                            stock_info.prev_close.clone(),
                            stock_info.datetime.clone(),
                        ],
                    )
                    .await;
                }
            }

            log(&msg_tx_clone, reply_clone, Info, format!("[{NAME}] update")).await;
        });
    }

    async fn show(&mut self, cmd: &Cmd) {
        log(
            &self.msg_tx,
            cmd.reply.clone(),
            Info,
            format!(
                "{:<4} {:<8} {:<18} {:<7} {:<12}  {:<7}  {:<7}",
                "Code", "Name", "Update", "Last", "Change", "Low", "High"
            ),
        )
        .await;

        for stock in &self.stocks {
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
            let mut info = String::new();

            info.push_str(&format!("{:<4} {} ", stock.code, stock.name));
            let width: usize = stock.name.chars().map(|c| c.width().unwrap_or(0)).sum();
            info.push_str(" ".repeat(8 - width).as_str());
            info.push_str(&format!("{:<18} {last_price:<7.2} {change:>5.2} {change_percent:>5.2}%  {low_price:<7.2}  {high_price:<7.2}", stock.datetime));

            log(&self.msg_tx, cmd.reply.clone(), Info, info.to_string()).await;
        }
    }

    async fn stock(&mut self, cmd: &Cmd) {
        let code = cmd.data[0].clone();
        let stock = self
            .stocks
            .iter_mut()
            .find(|p| p.code == code)
            .unwrap_or_else(|| panic!("Stock not found: {}", code));

        stock.name = cmd.data[1].clone();
        stock.last_price = cmd.data[2].clone();
        stock.high_price = cmd.data[3].clone();
        stock.low_price = cmd.data[4].clone();
        stock.prev_close = cmd.data[5].clone();
        stock.datetime = cmd.data[6].clone();

        msg::stocks(&self.msg_tx, self.stocks.clone()).await;
    }

    async fn help(&mut self) {
        info!(
            &self.msg_tx,
            format!(
                "[{NAME}] help: {}, {}, {}, {}",
                msg::ACT_INIT,
                msg::ACT_SHOW,
                msg::ACT_UPDATE,
                msg::ACT_STOCK
            )
        );
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
                msg::ACT_UPDATE => self.update(cmd).await,
                msg::ACT_STOCK => self.stock(cmd).await,
                _ => {
                    unknown!(&self.msg_tx, NAME, cmd.action);
                }
            },
            _ => {
                unknown!(&self.msg_tx, NAME, msg);
            }
        }

        false
    }
}
