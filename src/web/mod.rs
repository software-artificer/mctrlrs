mod middleware;
mod route;
mod session;
mod template;

use crate::core;
use actix_session::config;
use actix_web::{
    cookie::{self, time},
    error, http, web,
};
use std::{io, net};
use tokio::runtime;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to build Tokio runtime")]
    BuildRuntime(#[from] io::Error),
    #[error("Failed to listen on the {socket}")]
    BindServer {
        socket: net::SocketAddr,
        source: io::Error,
    },
    #[error("Failed to load handlebars template")]
    Template(#[from] handlebars::TemplateError),
}

pub fn start_server(config: core::Config) -> Result<(), Error> {
    runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(run_server(config))
}

fn internal_server_error() -> error::InternalError<&'static str> {
    error::InternalError::new(
        "Something Went Wrong",
        http::StatusCode::INTERNAL_SERVER_ERROR,
    )
}

fn redirect<P: AsRef<str>>(path: P) -> actix_web::HttpResponse {
    actix_web::HttpResponse::Found()
        .insert_header((http::header::LOCATION, path.as_ref()))
        .finish()
}

async fn run_server(config: core::Config) -> Result<(), Error> {
    println!("Starting webserver on {}", config.listen_on);

    let mut templates = handlebars::Handlebars::new();
    templates.register_templates_directory(
        "./templates/",
        handlebars::DirectorySourceOptions::default(),
    )?;
    let templates = web::Data::new(templates);
    let secret_key = cookie::Key::generate();
    let session_store = session::SessionStore::default();
    let app_config = web::Data::new(config.app_config);

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(templates.clone())
            .app_data(app_config.clone())
            .service(actix_files::Files::new("/static", "./static/"))
            .wrap(middleware::ConditionalMiddleware::new(
                middleware::AuthMiddleware::<session::UserSession>::new("/login"),
                |req: &actix_web::dev::ServiceRequest| {
                    !["/static", "/enroll", "/login"]
                        .iter()
                        .any(|path| req.path().starts_with(path))
                },
            ))
            .wrap(
                actix_session::SessionMiddleware::builder(
                    session_store.clone(),
                    secret_key.clone(),
                )
                .cookie_http_only(true)
                .cookie_same_site(cookie::SameSite::Strict)
                .session_lifecycle(config::SessionLifecycle::BrowserSession(
                    config::BrowserSession::default()
                        .state_ttl(time::Duration::minutes(15))
                        .state_ttl_extension_policy(config::TtlExtensionPolicy::OnEveryRequest),
                ))
                .build(),
            )
            .route("/", web::get().to(route::index_get))
            .route("/login", web::get().to(route::login_get))
            .route("/login", web::post().to(route::login_post))
            .route("/enroll", web::get().to(route::enroll_get))
            .route("/enroll", web::post().to(route::enroll_post))
            .route("/worlds", web::get().to(route::worlds_get))
            .route("/worlds", web::post().to(route::worlds_post))
    })
    .bind(config.listen_on)
    .map_err(|err| Error::BindServer {
        socket: config.listen_on,
        source: err,
    })?
    .run()
    .await?;

    Ok(())
}
