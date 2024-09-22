use crate::{
    core::server,
    web::{session, template},
};
use actix_web::web;

#[derive(serde::Serialize)]
struct IndexContent {
    players: Vec<String>,
    player_summary: String,
    tick_stats: Option<server::TickStats>,
}

pub async fn get(
    templates: web::Data<handlebars::Handlebars<'_>>,
    flash_messages: session::FlashMessages,
    client: web::Data<server::Client>,
) -> impl actix_web::Responder {
    let (player_summary, players) = match client.list().await {
        Ok(players) => {
            let summary = match players.len() {
                0 => "There are no players online".to_string(),
                1 => "There is 1 player online".to_string(),
                len => format!("There are {len} players online"),
            };

            (summary, players)
        }
        Err(err) => {
            tracing::error!("Failed to get the list of players: {err}");

            flash_messages.error("Failed to communicate with the Minecraft server.");

            (
                String::from("Unable to fetch a list of online players"),
                vec![],
            )
        }
    };

    let tick_stats = match client.query_tick().await {
        Ok(stats) => Some(stats),
        Err(err) => {
            tracing::error!("Failed to query tick stats from the server: {err}");

            flash_messages.error("Failed to fetch tick stats from the Minecraft server.");

            None
        }
    };

    let content = IndexContent {
        player_summary,
        players,
        tick_stats,
    };

    let content =
        template::Content::new(flash_messages, content).with_menu(template::ActiveMenu::Home);

    template::render_response(templates.as_ref(), "index", &content)
}
