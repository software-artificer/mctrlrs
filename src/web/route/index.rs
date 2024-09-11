use crate::{
    core::server,
    web::{session, template},
};
use actix_web::web;

#[derive(serde::Serialize)]
struct IndexContent {
    players: Vec<String>,
    summary: String,
}

pub async fn get(
    templates: web::Data<handlebars::Handlebars<'_>>,
    flash_messages: session::FlashMessages,
    client: web::Data<server::Client>,
) -> impl actix_web::Responder {
    let content = match client.list().await {
        Ok(players) => {
            let summary = match players.len() {
                0 => "There are no players online".to_string(),
                1 => "There is 1 player online".to_string(),
                len => format!("There are {len} players online"),
            };

            IndexContent { summary, players }
        }
        Err(err) => {
            eprintln!("Failed to get the list of players: {err}");

            IndexContent {
                summary: String::from("Failed to get the list of players"),
                players: vec![],
            }
        }
    };

    let content =
        template::Content::new(flash_messages, content).with_menu(template::ActiveMenu::Home);

    template::render_response(templates.as_ref(), "index", &content)
}
