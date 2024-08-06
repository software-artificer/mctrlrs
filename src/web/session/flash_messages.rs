use std::future;

use actix_session::SessionExt;
use actix_web::dev;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Info,
    Warning,
    Error,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct FlashMessage {
    pub message: String,
    level: Level,
}

pub struct FlashMessages(actix_session::Session);

impl FlashMessages {
    const FLASH_MESSAGES_KEY: &'static str = "flash_messages";

    pub fn info<M: AsRef<str>>(&self, message: M) {
        self.add(FlashMessage {
            message: message.as_ref().to_string(),
            level: Level::Info,
        });
    }

    pub fn error<M: AsRef<str>>(&self, message: M) {
        self.add(FlashMessage {
            message: message.as_ref().to_string(),
            level: Level::Error,
        });
    }

    pub fn warning<M: AsRef<str>>(&self, message: M) {
        self.add(FlashMessage {
            message: message.as_ref().to_string(),
            level: Level::Warning,
        });
    }

    pub fn take(&self) -> Vec<FlashMessage> {
        match self.0.remove_as(Self::FLASH_MESSAGES_KEY) {
            Some(Err(err)) => {
                eprintln!("Failed to fetch flash messages from session: {err}");

                vec![]
            }
            Some(Ok(flash_messages)) => flash_messages,
            None => {
                vec![]
            }
        }
    }

    fn add(&self, flash_message: FlashMessage) {
        let flash_messages = match self.0.get::<Vec<FlashMessage>>(Self::FLASH_MESSAGES_KEY) {
            Ok(Some(mut flash_messages)) => {
                flash_messages.push(flash_message);
                flash_messages
            }
            Ok(None) => {
                vec![flash_message]
            }
            Err(err) => {
                eprintln!("Failed to load flash messages from session: {err}");

                return;
            }
        };

        if let Err(err) = self.0.insert(Self::FLASH_MESSAGES_KEY, flash_messages) {
            eprintln!("Failed to save flash messages into session: {err}");
        }
    }
}

impl actix_web::FromRequest for FlashMessages {
    type Error = <actix_session::Session as actix_web::FromRequest>::Error;
    type Future = future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        future::ready(Ok(FlashMessages(req.get_session())))
    }
}
