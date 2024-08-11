use anyhow::Context;

use crate::core::{self, server};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to load a list of worlds: {0}")]
    LoadWorlds(#[source] core::WorldError),
    #[error("Failed to switch an active world: {0}")]
    Switch(#[source] anyhow::Error),
}

pub fn list(config: core::AppConfig) -> Result<(), Error> {
    let worlds = core::Worlds::new(&config.worlds_path, &config.current_world_path)
        .map_err(Error::LoadWorlds)?;

    println!("The following worlds are currently available:");
    for world in worlds.list() {
        if world.is_active {
            print!("> ");
        } else {
            print!("  ");
        }

        println!("{}", world.id());
    }

    Ok(())
}

pub fn switch(config: core::AppConfig, world_name: String) -> Result<(), Error> {
    let worlds = core::Worlds::new(&config.worlds_path, &config.current_world_path)
        .map_err(Error::LoadWorlds)?;

    let mut client = server::Client::new(config.rcon_address, config.rcon_password)
        .map_err(|e| Error::Switch(e.into()))?;
    client
        .save_all()
        .with_context(|| "Failed to save the world before switching")
        .map_err(Error::Switch)?;
    client
        .stop()
        .with_context(|| "Failed to shut down the server before switching")
        .map_err(Error::Switch)?;

    let world = worlds
        .switch(world_name)
        .map_err(|e| Error::Switch(e.into()))?;

    println!("The currently active world was changed to: {}", world.id(),);

    Ok(())
}
