use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use log::Level::{Info, Trace};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Sender};

const API_V1_CMD: &str = "/api/v1/cmd";

use crate::{
    cfg,
    msg::{self, log, Msg, Reply},
    plugins::plugin_todos,
};

const NAME: &str = "web";
const LISTENING_ON: (&str, u16) = ("0.0.0.0", 9759);

#[derive(Serialize, Deserialize, Debug)]
struct Cmd {
    cmd: String,
}

async fn cmd(req_body: String, sender: web::Data<Sender<Msg>>) -> impl Responder {
    let (resp_tx, resp_rx) = mpsc::channel::<Vec<String>>(100);

    let cmd: Cmd = serde_json::from_str(&req_body).unwrap();
    let cmd_args: Vec<&str> = cmd.cmd.split_whitespace().collect();
    let data: Vec<String> = cmd_args[3..].iter().map(|&s| s.to_string()).collect();

    log(
        &sender,
        Reply::Device(cfg::name()),
        Trace,
        format!("[{NAME}] {API_V1_CMD}: {}", cmd.cmd),
    )
    .await;

    let plugin = cmd_args[1];
    let action = cmd_args[2];

    msg::cmd(
        &sender,
        Reply::Web(resp_tx.clone()),
        plugin.to_owned(),
        action.to_owned(),
        data,
    )
    .await;

    let responses = match plugin {
        "todos" => match action {
            "show" => Some(plugin_todos::show(resp_rx).await),
            _ => None,
        },
        _ => None,
    };

    HttpResponse::Ok().json(responses.unwrap())
}

pub async fn run(msg_tx: Sender<Msg>) -> Result<(), Box<dyn std::error::Error>> {
    let msg_tx_clone = msg_tx.clone();
    tokio::spawn(async move {
        let _ = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(msg_tx_clone.clone()))
                .route(API_V1_CMD, web::post().to(cmd))
                .service(Files::new("/", "../client/out").index_file("index.html"))
        })
        .bind(LISTENING_ON)
        .unwrap()
        .run()
        .await;
    });

    log(
        &msg_tx,
        Reply::Device(cfg::name()),
        Info,
        format!("[{NAME}] init"),
    )
    .await;

    Ok(())
}
