use crate::{
    core,
    web::{self as core_web, middleware::AuthSession, session, template},
};
use actix_web::web;
use std::fmt;

#[derive(serde::Serialize)]
struct LoginForm {}

pub async fn get(
    templates: web::Data<handlebars::Handlebars<'_>>,
    flash_messages: session::FlashMessages,
    user_session: session::UserSession,
) -> impl actix_web::Responder {
    match user_session.is_authenticated() {
        Ok(true) => {
            user_session.purge();

            Ok(core_web::redirect("/login"))
        }
        Ok(false) => {
            let data = template::Content::new(flash_messages, LoginForm {});

            template::render_response(&templates, "login", &data)
        }
        Err(err) => {
            eprintln!("Failed to render the login page: {err}");

            Err(core_web::internal_server_error())
        }
    }
}

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    username: String,
    password: secrecy::SecretString,
}

pub async fn post(
    request: web::Form<LoginRequest>,
    flash_messages: session::FlashMessages,
    config: web::Data<core::AppConfig>,
    session: session::UserSession,
) -> impl actix_web::Responder {
    let request = request.into_inner();
    match request.username.try_into() {
        Ok(username) => match core::Users::load(&config.users_file_path) {
            Ok(users) => match users.find_user_by_username(&username) {
                Some(user) => match user.verify_password(request.password) {
                    core::PasswordVerifyResult::Valid => {
                        if session.authenticate(user).is_err() {
                            Err(internal_server_error("Failed to update the session state"))
                        } else {
                            Ok(core_web::redirect(session.get_redirect_location()))
                        }
                    }
                    core::PasswordVerifyResult::Error(err) => Err(internal_server_error(format!(
                        "Failed to parse PHC hash for the `{}` password: {err}",
                        user.username
                    ))),
                    _ => Ok(bad_credentials(&flash_messages)),
                },
                _ => Ok(bad_credentials(&flash_messages)),
            },
            Err(err) => Err(internal_server_error(format!(
                "Failed to load users: {err}"
            ))),
        },
        _ => Ok(bad_credentials(&flash_messages)),
    }
}

fn bad_credentials(flash_messages: &session::FlashMessages) -> actix_web::HttpResponse {
    flash_messages.error("Invalid username or password. Please try again.");

    core_web::redirect("/login")
}

fn internal_server_error(log: impl fmt::Display) -> actix_web::Error {
    eprintln!("{log}");

    core_web::internal_server_error().into()
}
