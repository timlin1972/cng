use actix_files::Files;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use log::Level::Info;
use tokio::sync::mpsc::Sender;

use crate::{
    cfg,
    msg::{log, Msg},
};

const NAME: &str = "web";
const LISTENING_ON: (&str, u16) = ("0.0.0.0", 9759);

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

pub async fn run(msg_tx: Sender<Msg>) -> Result<(), Box<dyn std::error::Error>> {
    tokio::spawn(async move {
        let _ = HttpServer::new(|| {
            App::new()
                .route("/hey", web::get().to(manual_hello))
                .service(Files::new("/", "../client/out").index_file("index.html"))
        })
        .bind(LISTENING_ON)
        .unwrap()
        .run()
        .await;
    });

    log(&msg_tx, cfg::name(), Info, format!("[{NAME}] init")).await;

    Ok(())
}
