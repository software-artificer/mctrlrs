use actix_session::SessionExt;
use actix_web::dev;
use std::future;

use crate::web::middleware;

pub struct UserSession(actix_session::Session);

impl UserSession {
    const USERNAME_KEY: &'static str = "username";
    const REDIRECT_LOCATION_KEY: &'static str = "location";

    pub fn purge(&self) {
        self.0.purge();
    }
}

impl middleware::AuthSession for UserSession {
    type IsAuthenticatedError = actix_session::SessionGetError;
    type SaveRedirectError = actix_session::SessionInsertError;

    fn is_authenticated(&self) -> Result<bool, Self::IsAuthenticatedError> {
        self.0
            .get::<String>(Self::USERNAME_KEY)
            .map(|username| username.is_some())
    }

    fn save_redirect(&self, location: String) -> Result<(), Self::SaveRedirectError> {
        self.0
            .insert::<String>(Self::REDIRECT_LOCATION_KEY, location)
    }
}

impl actix_web::FromRequest for UserSession {
    type Error = <actix_session::Session as actix_web::FromRequest>::Error;
    type Future = future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        future::ready(Ok(UserSession(req.get_session())))
    }
}
