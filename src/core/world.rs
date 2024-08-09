use std::{fs, io, path};

pub struct World {
    pub id: path::PathBuf,
    pub is_active: bool,
}

pub struct Worlds {
    worlds: Vec<World>,
}

impl Worlds {
    pub fn new(
        worlds_path: &path::Path,
        current_world_name: &path::Path,
    ) -> Result<Self, WorldError> {
        let current_world = current_world_name
            .read_link()
            .map_err(WorldError::ReadWorldDir)?;

        let mut worlds = vec![];

        let entries = fs::read_dir(worlds_path).map_err(WorldError::ReadWorldDir)?;
        for entry in entries {
            let entry = entry.map_err(WorldError::ReadWorldDir)?;
            let entry_path = entry.path();

            if !entry_path.is_dir() || entry_path.is_symlink() {
                continue;
            }

            let entry_name = entry_path
                .file_name()
                .expect("Read the directory entry without a file name");
            let entry_name: &path::Path = entry_name.as_ref();

            worlds.push(World {
                id: entry_name.to_owned(),
                is_active: entry_path == current_world,
            });
        }

        Ok(Self { worlds })
    }

    pub fn list(&self) -> &Vec<World> {
        &self.worlds
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WorldError {
    #[error("Unable to read worlds directory: {}", .0)]
    ReadWorldDir(#[source] io::Error),
}
