use crate::core;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to load a list of worlds: {}", .0)]
    LoadWorlds(#[source] core::WorldError),
    #[error("Failed to switch an active world: {}", .0)]
    Switch(#[source] core::WorldError),
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

        println!("{}", world.id.display());
    }

    Ok(())
}

pub fn switch(config: core::AppConfig, world_name: String) -> Result<(), Error> {
    let worlds = core::Worlds::new(&config.worlds_path, &config.current_world_path)
        .map_err(Error::LoadWorlds)?;

    let world = worlds.switch(world_name).map_err(Error::Switch)?;

    println!(
        "The currently active world was changed to: {}",
        world.id.display(),
    );

    Ok(())
}
