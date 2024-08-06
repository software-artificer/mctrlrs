use crate::web::{self as core_web, middleware::AuthSession, session, template};
use actix_web::web;

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

            flash_messages.info("You've been successfully logged out");
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

pub async fn post() -> impl actix_web::Responder {
    actix_web::HttpResponse::NotImplemented().finish()
}
