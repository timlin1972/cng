use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use log::Level::Info;
use tokio::sync::mpsc::Sender;

use crate::{
    cfg,
    msg::{log, Msg},
};

const NAME: &str = "web";
const LISTENING_ON: (&str, u16) = ("0.0.0.0", 9759);

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

pub async fn run(msg_tx: Sender<Msg>) -> Result<(), Box<dyn std::error::Error>> {
    tokio::spawn(async move {
        let _ = HttpServer::new(|| {
            App::new()
                .service(hello)
                .service(echo)
                .route("/hey", web::get().to(manual_hello))
        })
        .bind(LISTENING_ON)
        .unwrap()
        .run()
        .await;
    });

    log(&msg_tx, cfg::name(), Info, format!("[{NAME}] init")).await;

    Ok(())
}
