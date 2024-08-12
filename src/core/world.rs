use super::properties;
use std::{fs, io, path};

pub struct World {
    id: path::PathBuf,
    pub is_active: bool,
}

impl World {
    pub fn id(&self) -> String {
        format!("{}", self.id.display())
    }
}

pub struct Worlds {
    worlds: Vec<World>,
    properties: properties::Properties,
    current_world_name: String,
}

impl Worlds {
    pub fn new(
        worlds_path: &path::Path,
        server_properties_path: &path::Path,
    ) -> Result<Self, WorldError> {
        let properties = properties::Properties::parse(server_properties_path)
            .map_err(WorldError::LoadServerProperties)?;
        let current_world_name = properties.level_name();
        let current_world = path::PathBuf::from(&current_world_name);

        let mut worlds = vec![];

        let entries = fs::read_dir(worlds_path).map_err(WorldError::ReadWorldDir)?;
        for entry in entries {
            let entry = entry.map_err(WorldError::ReadWorldDir)?;
            let entry_path = entry.path();

            if !entry_path.is_dir() {
                continue;
            }

            let entry_name = entry_path
                .file_name()
                .expect("Read the directory entry without a file name");
            let entry_name: &path::Path = entry_name.as_ref();

            worlds.push(World {
                id: entry_name.to_owned(),
                is_active: entry_name == current_world,
            });
        }

        Ok(Self {
            worlds,
            properties,
            current_world_name,
        })
    }

    pub fn list(&self) -> &Vec<World> {
        &self.worlds
    }

    pub fn switch(self, world_name: String) -> Result<World, WorldError> {
        if self.current_world_name == world_name {
            Err(WorldError::AlreadyActive(world_name))
        } else {
            let world_id = path::PathBuf::from(&world_name);

            match self.worlds.into_iter().find(|world| world.id == world_id) {
                Some(world) => {
                    self.properties
                        .with_level_name(world_name)
                        .map_err(WorldError::Switch)?;

                    Ok(world)
                }
                _ => Err(WorldError::NoSuchWorld(world_id)),
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WorldError {
    #[error("Unable to read worlds directory: {0}")]
    ReadWorldDir(#[source] io::Error),
    #[error("No world with id `{}`", .0.display())]
    NoSuchWorld(path::PathBuf),
    #[error("World `{0}` is already active")]
    AlreadyActive(String),
    #[error("Failed to switch the world: {0}")]
    Switch(#[source] properties::Error),
    #[error("Failed to load server.properties file: {0}")]
    LoadServerProperties(#[source] properties::Error),
}
