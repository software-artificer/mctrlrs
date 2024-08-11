mod rcon;

use std::net;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Connect(#[source] rcon::RconError),
    #[error("{0}")]
    Authenticate(#[source] rcon::RconError),
    #[error("Failed to save the world: {0}")]
    SaveAll(#[source] rcon::RconError),
    #[error("Failed to stop the server: {0}")]
    Stop(#[source] rcon::RconError),
    #[error("Lost Minecraft server connection: {0}")]
    BrokenConnection(#[source] rcon::RconError),
}

pub struct Client(rcon::RconClient<rcon::Authenticated>);

impl Client {
    pub fn new(addr: net::SocketAddr, password: secrecy::SecretString) -> Result<Self, Error> {
        Ok(Self(
            rcon::RconClient::new()
                .connect(addr)
                .map_err(Error::Connect)?
                .authenticate(password)
                .map_err(Error::Authenticate)?,
        ))
    }

    pub fn save_all(&mut self) -> Result<(), Error> {
        run_command(&mut self.0, "save-all", Error::SaveAll)?;

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        run_command(&mut self.0, "stop", Error::Stop)?;

        Ok(())
    }
}

fn run_command<F, C>(
    client: &mut rcon::RconClient<rcon::Authenticated>,
    command: C,
    err_func: F,
) -> Result<String, Error>
where
    C: AsRef<str>,
    F: FnOnce(rcon::RconError) -> Error,
{
    client
        .command(command.as_ref().to_string())
        .map_err(|e| match e {
            e @ rcon::RconError::Read(_) | e @ rcon::RconError::Write(_) => {
                Error::BrokenConnection(e)
            }
            e => err_func(e),
        })
}
