use std::{fs, io, os::unix::fs as ufs, path};

pub struct World {
    pub id: path::PathBuf,
    pub is_active: bool,
    path: path::PathBuf,
}

pub struct Worlds {
    worlds: Vec<World>,
    current_world: path::PathBuf,
}

impl Worlds {
    pub fn new(
        worlds_path: &path::Path,
        current_world_path: &path::Path,
    ) -> Result<Self, WorldError> {
        let current_world = current_world_path
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
                path: entry_path,
            });
        }

        Ok(Self {
            worlds,
            current_world: current_world_path.to_owned(),
        })
    }

    pub fn list(&self) -> &Vec<World> {
        &self.worlds
    }

    pub fn switch(self, world_name: String) -> Result<World, WorldError> {
        let world_id: path::PathBuf = world_name.into();

        match self.worlds.into_iter().find(|world| world.id == world_id) {
            Some(world) => {
                if self.current_world.is_symlink() && !world.is_active {
                    fs::remove_file(&self.current_world).map_err(WorldError::Switch)?;
                    ufs::symlink(&world.path, &self.current_world).map_err(WorldError::Switch)?;
                }

                Ok(world)
            }
            _ => Err(WorldError::NoSuchWorld(world_id)),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WorldError {
    #[error("Unable to read worlds directory: {}", .0)]
    ReadWorldDir(#[source] io::Error),
    #[error("No world named `{}`", .0.display())]
    NoSuchWorld(path::PathBuf),
    #[error("Failed to switch the world: {}", .0)]
    Switch(#[source] io::Error),
}
