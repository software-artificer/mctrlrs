use crate::{
    core::{self, server},
    web::{self, session, template},
};
use actix_web::web as aweb;

type WorldsList = Vec<World>;

#[derive(serde::Serialize)]
struct World {
    id: String,
    is_current: bool,
    name: String,
}

impl From<core::Worlds> for WorldsList {
    fn from(worlds: core::Worlds) -> Self {
        let mut list = vec![];

        for world in worlds.list() {
            list.push(World {
                name: id_to_name(&world.id()),
                id: world.id(),
                is_current: world.is_active,
            })
        }

        list.sort_by(|a, b| a.name.cmp(&b.name));

        list
    }
}

fn id_to_name(id: &str) -> String {
    id.split('_')
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case_word(word: &str) -> String {
    word.chars()
        .enumerate()
        .fold(String::with_capacity(word.len()), |mut s, (idx, chr)| {
            if idx == 0 {
                s += &chr.to_uppercase().to_string();
            } else {
                s.push(chr);
            }

            s
        })
}

pub async fn get(
    config: aweb::Data<core::AppConfig>,
    templates: aweb::Data<handlebars::Handlebars<'_>>,
    flash_messages: session::FlashMessages,
) -> impl actix_web::Responder {
    match core::Worlds::new(&config.worlds_path, &config.current_world_path) {
        Ok(worlds) => {
            let worlds: WorldsList = worlds.into();
            let content = template::Content::new(flash_messages, worlds)
                .with_menu(template::ActiveMenu::Worlds);

            template::render_response(&templates, "worlds", &content)
        }
        Err(err) => {
            eprintln!("Failed to load worlds: {err}");

            Err(web::internal_server_error())
        }
    }
}

#[derive(serde::Deserialize)]
pub struct WorldSwitchForm {
    world_id: String,
}

pub async fn post(
    config: aweb::Data<core::AppConfig>,
    request: aweb::Form<WorldSwitchForm>,
    flash_messages: session::FlashMessages,
) -> impl actix_web::Responder {
    match core::Worlds::new(&config.worlds_path, &config.current_world_path) {
        Ok(worlds) => {
            match server::Client::new(config.rcon_address, config.rcon_password.clone()) {
                Ok(mut client) => {
                    if let Err(err) = client.save_all() {
                        eprintln!("{err}");

                        flash_messages.error("Failed to save the current world.");

                        Ok(web::redirect("/worlds"))
                    } else if let Err(err) = client.stop() {
                        eprintln!("{err}");

                        flash_messages.error("Failed to stop the Minecraft server.");

                        Ok(web::redirect("/worlds"))
                    } else {
                        flash_messages.warning("The Minecraft server was restarted.");

                        match worlds.switch(request.world_id.to_string()) {
                            Ok(world) => {
                                flash_messages.info(format!(
                                    r#""{}" is now the active world."#,
                                    id_to_name(&world.id())
                                ));

                                Ok(web::redirect("/worlds"))
                            }
                            Err(core::WorldError::NoSuchWorld(id)) => {
                                flash_messages.error(format!(
                                    r#"World with id "{}" is not available."#,
                                    id.display()
                                ));

                                Ok(web::redirect("/worlds"))
                            }
                            Err(err) => {
                                eprintln!("Failed to switch the world: {err}");

                                Err(web::internal_server_error())
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Failed to create an RCON client: {err}");

                    flash_messages.error("Unable to connect to the Minecraft server.");

                    Ok(web::redirect("/worlds"))
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to load worlds: {err}");

            Err(web::internal_server_error())
        }
    }
}
