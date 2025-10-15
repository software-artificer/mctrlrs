use crate::web::{self, session};
use actix_web::{error, http::header};

#[derive(serde::Serialize)]
pub struct Content<C: serde::Serialize> {
    app_version: &'static str,
    content: C,
    flash_messages: Vec<session::FlashMessage>,
    menu: ActiveMenu,
}

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default)]
pub enum ActiveMenu {
    #[default]
    None,
    Home,
    Worlds,
}

impl serde::Serialize for ActiveMenu {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            Self::None => "",
            Self::Home => "home",
            Self::Worlds => "worlds",
        };

        String::serialize(&value.to_string(), serializer)
    }
}

impl<C: serde::Serialize> Content<C> {
    pub fn new(flash_messages: session::FlashMessages, content: C) -> Self {
        Self {
            content,
            app_version: APP_VERSION,
            flash_messages: flash_messages.take(),
            menu: Default::default(),
        }
    }

    pub fn with_menu(self, active_item: ActiveMenu) -> Self {
        Self {
            menu: active_item,
            ..self
        }
    }
}

pub fn render_template<N: AsRef<str>, C: serde::Serialize>(
    templates: &handlebars::Handlebars,
    name: N,
    data: &Content<C>,
) -> Result<String, error::Error> {
    match templates.render(name.as_ref(), data) {
        Ok(content) => Ok(content),
        Err(err) => {
            tracing::error!("Failed to render Handlebar template: {err}");

            Err(web::internal_server_error().into())
        }
    }
}

pub fn render_response<N: AsRef<str>, C: serde::Serialize>(
    templates: &handlebars::Handlebars,
    name: N,
    data: &Content<C>,
) -> Result<actix_web::HttpResponse, error::Error> {
    render_template(templates, name, data).map(|content| {
        actix_web::HttpResponse::Ok()
            .content_type(header::ContentType::html())
            .body(content)
    })
}
