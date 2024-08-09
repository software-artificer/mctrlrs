use crate::web::{session, template};
use actix_web::web;

#[derive(serde::Serialize)]
struct IndexContent;

pub async fn get(
    templates: web::Data<handlebars::Handlebars<'_>>,
    flash_messages: session::FlashMessages,
) -> impl actix_web::Responder {
    let content =
        template::Content::new(flash_messages, IndexContent).with_menu(template::ActiveMenu::Home);

    template::render_response(templates.as_ref(), "index", &content)
}
