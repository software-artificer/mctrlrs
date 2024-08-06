use crate::web::{
    self as core_web, core, internal_server_error, middleware::AuthSession, session, template,
};
use actix_web::web;

#[derive(serde::Deserialize)]
pub struct Parameters {
    token: String,
}

enum TokenState {
    Valid(String),
    Invalid,
    Error,
}

#[derive(serde::Serialize)]
struct EnrollForm {
    token: String,
    username: String,
}

pub async fn get(
    session: session::UserSession,
    query: web::Query<Parameters>,
    templates: web::Data<handlebars::Handlebars<'_>>,
    config: web::Data<core::AppConfig>,
    flash_messages: session::FlashMessages,
) -> impl actix_web::Responder {
    let query = query.into_inner();
    match session.is_authenticated() {
        Ok(true) => {
            flash_messages.warning("You are already authenticated, no need to re-enroll.");

            Ok(core_web::redirect("/"))
        }
        Ok(false) => match validate_token(&config.into_inner(), &query.token) {
            TokenState::Valid(username) => {
                let content = template::Content::new(
                    flash_messages,
                    EnrollForm {
                        token: query.token,
                        username,
                    },
                );
                template::render_response(&templates, "enroll", &content)
            }
            TokenState::Invalid => {
                flash_messages.error("Provided enroll token is invalid.");
                Ok(core_web::redirect("/login"))
            }
            TokenState::Error => Err(core_web::internal_server_error()),
        },
        Err(err) => {
            eprintln!("Failed to fetch session state: {err}");

            Err(core_web::internal_server_error())
        }
    }
}

fn validate_token(config: &core::AppConfig, token: &str) -> TokenState {
    let token_result = token.try_into();
    match token_result {
        Ok(token) => match core::Users::load(&config.users_file_path) {
            Ok(users) => {
                if let Some(username) = users.find_username_by_token(token) {
                    TokenState::Valid(username.to_string())
                } else {
                    TokenState::Invalid
                }
            }
            Err(err) => {
                eprintln!("Failed to load users to verify enroll token: {err}");

                TokenState::Error
            }
        },
        _ => TokenState::Invalid,
    }
}

#[derive(serde::Deserialize)]
pub struct EnrollRequest {
    token: String,
    password: String,
    repassword: String,
}

pub async fn post(
    request: web::Form<EnrollRequest>,
    flash_messages: session::FlashMessages,
    config: web::Data<core::AppConfig>,
) -> impl actix_web::Responder {
    let request = request.into_inner();

    match verify_password(&config, request.password, request.repassword) {
        Ok(password) => match change_password(&config, request.token, password) {
            EnrollResult::Ok => {
                flash_messages.info("The user was successfully enrolled.");
                Ok(core_web::redirect("/login"))
            }
            EnrollResult::BadToken => {
                flash_messages.error("Provided enroll token is invalid.");
                Ok(core_web::redirect("/login"))
            }
            EnrollResult::Other(reason) => {
                eprintln!("Failed to enroll the user: {reason}");

                Err(internal_server_error())
            }
        },
        Err(err) => match err {
            PasswordError::HashFailed(error) => {
                eprintln!("Failed to hash the password: {}", error);

                Err(internal_server_error())
            }
            PasswordError::BadPassword(err) => {
                flash_messages.error(err);
                Ok(core_web::redirect(format!(
                    "/enroll?token={}",
                    request.token
                )))
            }
        },
    }
}

enum EnrollResult {
    Ok,
    BadToken,
    Other(String),
}

fn change_password(
    config: &core::AppConfig,
    token: String,
    password: core::Password,
) -> EnrollResult {
    match token.try_into() {
        Ok(token) => match core::Users::load(&config.users_file_path) {
            Ok(users) => match users.find_username_by_token(token) {
                Some(username) => {
                    let username = username.clone();
                    if let Err(err) = users.update_password(&username, password) {
                        EnrollResult::Other(err.to_string())
                    } else {
                        EnrollResult::Ok
                    }
                }
                _ => EnrollResult::BadToken,
            },
            Err(err) => EnrollResult::Other(format!("{err}")),
        },
        _ => EnrollResult::BadToken,
    }
}

enum PasswordError {
    BadPassword(String),
    HashFailed(String),
}

impl From<core::PasswordError> for PasswordError {
    fn from(value: core::PasswordError) -> Self {
        match value {
            core::PasswordError::Short(len) => Self::BadPassword(format!(
                "A password must be longer than {} characters. Please use a longer password!",
                len
            )),
            core::PasswordError::Long(len) => Self::BadPassword(format!(
                "A password must be shorter than {} characters. Please use a shorter password!",
                len
            )),
            core::PasswordError::Weak => Self::BadPassword(
                "A password must contain a lowercase letter, an uppercase letter, \
            a digit and a punctuation character. Please use another password!"
                    .to_string(),
            ),
            core::PasswordError::Hash(err) => Self::HashFailed(err.to_string()),
        }
    }
}

fn verify_password(
    config: &core::AppConfig,
    password: String,
    repassword: String,
) -> Result<core::Password, PasswordError> {
    if password != repassword {
        Err(PasswordError::BadPassword(
            "Passwords do not match. Please try again!".to_string(),
        ))
    } else {
        Ok(core::Password::new(password, config)?)
    }
}
