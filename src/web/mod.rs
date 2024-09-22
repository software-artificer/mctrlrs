mod middleware;
mod route;
mod session;
mod template;

use crate::core::{self, server};
use actix_session::config;
use actix_web::{
    cookie::{self, time},
    error, http, web,
};
use std::{fs, io, net};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to listen on the {socket}")]
    BindServer {
        socket: net::SocketAddr,
        source: io::Error,
    },
    #[error("Failed to load handlebars template")]
    Template(#[from] handlebars::TemplateError),
    #[error("Actix web server failed: {0}")]
    Actix(#[from] std::io::Error),
    #[error("Failed to configure TLS: {0}")]
    Tls(String),
}

pub fn start_server(config: core::Config) -> Result<(), Error> {
    actix_web::rt::System::new().block_on(run_server(config))
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
    let client = web::Data::new(server::Client::new(
        app_config.rcon_address,
        app_config.rcon_password.clone(),
    ));

    let server = actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(templates.clone())
            .app_data(app_config.clone())
            .app_data(client.clone())
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
    });

    let server = if let Some(worker_count) = config.worker_count {
        server.workers(worker_count.get())
    } else {
        server
    };

    let server = if let Some(tls) = config.tls {
        let tls_config = configure_tls(tls).map_err(Error::Tls)?;
        server.bind_rustls_0_23(config.listen_on, tls_config)
    } else {
        server.bind(config.listen_on)
    }
    .map_err(|err| Error::BindServer {
        socket: config.listen_on,
        source: err,
    })?;

    server.run().await?;

    Ok(())
}

fn configure_tls(tls: core::TlsConfig) -> Result<rustls::ServerConfig, String> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| "Failed to install the default TLS provider to ring".to_string())?;

    let config = rustls::ServerConfig::builder().with_no_client_auth();

    let key_file = fs::File::open(&tls.key).map_err(|e| {
        format!(
            "Failed to open a private key file `{}`: {e}",
            tls.key.display()
        )
    })?;
    let key_file = &mut io::BufReader::new(key_file);

    let chain_file = fs::File::open(&tls.chain).map_err(|e| {
        format!(
            "Failed to open a certificate chain file `{}`: {e}",
            tls.chain.display()
        )
    })?;
    let chain_file = &mut io::BufReader::new(chain_file);

    let cert_chain = rustls_pemfile::certs(chain_file)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            format!(
                "Failed to parse a certificate chain file `{}`: {e}",
                tls.chain.display()
            )
        })?;

    let key = rustls_pemfile::private_key(key_file).map_err(|e| {
        format!(
            "Failed to parse a private key file `{}`: {e}",
            tls.key.display()
        )
    })?;
    let key = key.ok_or_else(|| {
        format!(
            "No keys found in a private key file `{}`",
            tls.key.display()
        )
    })?;

    config
        .with_single_cert(cert_chain, key)
        .map_err(|e| format!("Invalid certificate/key pair: {e}"))
}
