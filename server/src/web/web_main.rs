use std::fs::File;
use std::io::Write;

use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use futures_util::StreamExt;
use log::Level::{Info, Trace};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Sender};

use crate::{info, init, trace};

const API_V1_CMD: &str = "/api/v1/cmd";
const API_V1_UPLOAD: &str = "/api/v1/upload";

use crate::{
    cfg,
    msg::{self, log, Msg, Reply},
    utils,
};

const NAME: &str = "web";
const LISTENING_ON: (&str, u16) = ("0.0.0.0", 9759);

#[derive(Serialize, Deserialize, Debug)]
struct Cmd {
    cmd: String,
}

async fn cmd(req_body: String, sender: web::Data<Sender<Msg>>) -> impl Responder {
    let (resp_tx, mut resp_rx) = mpsc::channel::<serde_json::Value>(100);

    let cmd: Cmd = serde_json::from_str(&req_body).unwrap();
    let cmd_args: Vec<&str> = cmd.cmd.split_whitespace().collect();
    let data: Vec<String> = cmd_args[3..].iter().map(|&s| s.to_string()).collect();

    trace!(&sender, format!("[{NAME}] {API_V1_CMD}: {}", cmd.cmd));

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

    let response = resp_rx.recv().await.unwrap();
    HttpResponse::Ok().json(response)
}

async fn upload_file(mut payload: Multipart, sender: web::Data<Sender<Msg>>) -> impl Responder {
    while let Some(Ok(mut field)) = payload.next().await {
        let filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .map(sanitize_filename::sanitize)
            .unwrap_or_else(|| format!("upload-{}.bin", uuid::Uuid::new_v4()));

        let filepath = format!("{}/{filename}", cfg::UPLOAD_FOLDER);
        info!(&sender, format!("[{NAME}] [Go] {filepath}"));

        let start_ts = utils::ts();

        let mut f = match File::create(&filepath) {
            Ok(file) => file,
            Err(e) => {
                return HttpResponse::InternalServerError().body(format!("File error: {}", e))
            }
        };

        while let Some(Ok(chunk)) = field.next().await {
            if let Err(e) = f.write_all(&chunk) {
                return HttpResponse::InternalServerError().body(format!("Write error: {}", e));
            }
        }

        let escaped_time = utils::ts() - start_ts;
        info!(
            &sender,
            format!(
                "[{NAME}] [Ok] {filepath}, {}",
                utils::transmit_str(f.metadata().unwrap().len(), escaped_time)
            )
        );
    }

    HttpResponse::Ok().body("Upload complete")
}

pub async fn run(msg_tx: Sender<Msg>) -> Result<(), Box<dyn std::error::Error>> {
    let msg_tx_clone = msg_tx.clone();
    std::fs::create_dir_all(cfg::UPLOAD_FOLDER).unwrap();

    tokio::spawn(async move {
        let _ = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(msg_tx_clone.clone()))
                .route(API_V1_CMD, web::post().to(cmd))
                .route(API_V1_UPLOAD, web::post().to(upload_file))
                .service(Files::new("/shared", "./shared").show_files_listing())
                .service(Files::new("/", "../client/out").index_file("index.html"))
        })
        .bind(LISTENING_ON)
        .unwrap()
        .run()
        .await;
    });

    init!(&msg_tx, NAME);

    Ok(())
}
