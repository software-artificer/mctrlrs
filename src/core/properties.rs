use std::{
    collections, fs,
    io::{self, BufRead, Write},
    path,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to open server.properties file: {0}")]
    Open(#[source] io::Error),
    #[error("Failed to read the line from server.properties file: {0}")]
    Read(#[source] io::Error),
    #[error("Failed to write an updated server.properties file: {0}")]
    Write(#[source] io::Error),
    #[error("Broken server.properties file. Malformed line {0}")]
    MalformedLine(usize),
    #[error("The server.properties has an invalid rcon.port property or it is invalid")]
    InvalidRconPort,
    #[error("The server.properties does not contain an rcon.password property")]
    MissingRconPassword,
}

pub struct Properties {
    inner: collections::HashMap<String, String>,
    path: path::PathBuf,
}

impl Properties {
    const LEVEL_NAME_KEY: &'static str = "level-name";
    const RCON_PORT_KEY: &'static str = "rcon.port";
    const RCON_PASSWORD_KEY: &'static str = "rcon.password";

    pub fn parse(path: &path::Path) -> Result<Self, Error> {
        let path = path.to_owned();
        let file = fs::File::open(&path).map_err(Error::Open)?;
        let reader = io::BufReader::new(file);

        let mut inner = collections::HashMap::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(Error::Read)?;
            let line = line.trim();

            if line.starts_with('#') {
                continue;
            }

            let (key, value) = line.split_once('=').ok_or(Error::MalformedLine(line_num))?;
            let key = key.trim();
            let value = value.trim();
            inner.insert(key.to_string(), value.to_string());
        }

        Ok(Self { inner, path })
    }

    pub fn rcon_properties(&self) -> Result<RconProperties, Error> {
        let port: u16 = self
            .inner
            .get(Self::RCON_PORT_KEY)
            .ok_or(Error::InvalidRconPort)?
            .parse()
            .map_err(|_| Error::InvalidRconPort)?;

        let password = secrecy::SecretString::from(
            self.inner
                .get(Self::RCON_PASSWORD_KEY)
                .ok_or(Error::MissingRconPassword)?
                .to_string(),
        );

        Ok(RconProperties { port, password })
    }

    pub fn level_name(&self) -> String {
        self.inner
            .get(Self::LEVEL_NAME_KEY)
            .cloned()
            .unwrap_or("world".to_string())
    }

    pub fn with_level_name(mut self, world_name: String) -> Result<Self, Error> {
        self.inner
            .insert(Self::LEVEL_NAME_KEY.to_string(), world_name);

        let mut file = fs::File::create(&self.path).map_err(Error::Write)?;
        self.inner
            .iter()
            .map(|(key, value)| -> io::Result<()> {
                file.write_all(key.as_bytes())?;
                file.write_all("=".as_bytes())?;
                file.write_all(value.as_bytes())?;
                file.write_all("\n".as_bytes())?;

                Ok(())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Write)?;

        Ok(self)
    }
}

pub struct RconProperties {
    pub port: u16,
    pub password: secrecy::SecretString,
}
