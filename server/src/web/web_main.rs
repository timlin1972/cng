use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::task::{Context, Poll};

use actix_files::Files;
use actix_multipart::Multipart;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::CONTENT_TYPE,
    web, App, Error, HttpResponse, HttpServer, Responder,
};
use futures_util::future::{ok, LocalBoxFuture, Ready};
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

#[derive(Clone)]
struct CharsetMiddleware;

impl<S, B> Transform<S, ServiceRequest> for CharsetMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = CharsetMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CharsetMiddlewareService {
            service: Rc::new(service),
        })
    }
}

struct CharsetMiddlewareService<S> {
    service: Rc<S>,
}
impl<S, B> Service<ServiceRequest> for CharsetMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);

        Box::pin(async move {
            let mut res = service.call(req).await?;

            if let Some(content_type) = res.headers().get(CONTENT_TYPE) {
                if let Ok(content_type_str) = content_type.to_str() {
                    if content_type_str.starts_with("text/")
                        && !content_type_str.contains("charset")
                    {
                        let new_header = format!("{}; charset=utf-8", content_type_str);
                        res.headers_mut()
                            .insert(CONTENT_TYPE, new_header.parse().unwrap());
                    }
                }
            }

            Ok(res)
        })
    }
}

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
                .wrap(CharsetMiddleware)
                .service(
                    Files::new("/shared", "./shared")
                        .show_files_listing()
                        .prefer_utf8(true),
                )
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
