use crate::{
    core,
    web::{self, middleware},
};
use actix_session::SessionExt;
use actix_web::{dev, web as aweb};
use std::future;

pub struct UserSession {
    session: actix_session::Session,
    users: core::Users,
}

impl UserSession {
    const USERNAME_KEY: &'static str = "username";
    const REDIRECT_LOCATION_KEY: &'static str = "location";

    pub fn purge(&self) {
        self.session.purge();
    }

    pub fn get_current_user(&self) -> Result<Option<&core::User>, actix_session::SessionGetError> {
        match self.session.get::<String>(Self::USERNAME_KEY)? {
            Some(username) => match username.try_into() {
                Ok(username) => match self.users.find_user_by_username(&username) {
                    Some(user) => Ok(Some(user)),
                    _ => {
                        self.purge();

                        Ok(None)
                    }
                },
                _ => {
                    self.purge();

                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    pub fn authenticate(&self, user: &core::User) -> Result<(), actix_session::SessionInsertError> {
        self.session.renew();
        self.session
            .insert(Self::USERNAME_KEY, user.username.to_string())
    }

    pub fn get_redirect_location(&self) -> String {
        self.session
            .get::<String>(Self::REDIRECT_LOCATION_KEY)
            .unwrap_or(Some("/".to_string()))
            .unwrap_or("/".to_string())
    }
}

impl middleware::AuthSession for UserSession {
    type IsAuthenticatedError = actix_session::SessionGetError;
    type SaveRedirectError = actix_session::SessionInsertError;

    fn is_authenticated(&self) -> Result<bool, Self::IsAuthenticatedError> {
        self.get_current_user().map(|user| user.is_some())
    }

    fn save_redirect(&self, location: String) -> Result<(), Self::SaveRedirectError> {
        self.session
            .insert::<String>(Self::REDIRECT_LOCATION_KEY, location)
    }
}

impl actix_web::FromRequest for UserSession {
    type Error = <actix_session::Session as actix_web::FromRequest>::Error;
    type Future = future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let config = req
            .app_data::<aweb::Data<core::AppConfig>>()
            .expect("Application is misconfigured. Missing AppConfig struct.");

        match core::Users::load(&config.users_file_path) {
            Ok(users) => {
                let session = req.get_session();
                future::ready(Ok(UserSession { users, session }))
            }
            Err(err) => {
                tracing::error!("Unable to load users: {err}");

                future::ready(Err(web::internal_server_error().into()))
            }
        }
    }
}
