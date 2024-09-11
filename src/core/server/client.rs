use super::{actor, rcon};
use actix::Actor;
use std::net;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Connect(#[source] rcon::RconError),
    #[error("{0}")]
    Authenticate(#[source] rcon::RconError),
    #[error("Failed to execute the command: {0}")]
    Command(#[source] rcon::RconError),
    #[error("Lost Minecraft server connection: {0}")]
    BrokenConnection(#[source] rcon::RconError),
    #[error("Failed to send a message to the actor: {0}")]
    Actor(#[source] actix::MailboxError),
}

#[derive(Clone)]
pub struct Client(actix::Addr<actor::RconActor>);

impl Client {
    pub fn new(addr: net::SocketAddr, password: secrecy::SecretString) -> Self {
        let actor = actor::RconActor::new(addr, password);

        Self(actor.start())
    }

    pub async fn save_all(&self) -> Result<(), Error> {
        run_command(&self.0, actor::Command::Other("save-all".to_string())).await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Error> {
        run_command(&self.0, actor::Command::Stop).await?;

        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<String>, Error> {
        let list = run_command(&self.0, actor::Command::Other("list".to_string())).await?;

        Ok(match list.split_once(": ") {
            Some((_, players)) => {
                if players.is_empty() {
                    vec![]
                } else {
                    players.split(", ").map(|f| f.to_owned()).collect()
                }
            }
            None => vec![],
        })
    }
}

async fn run_command(
    actor: &actix::Addr<actor::RconActor>,
    command: actor::Command,
) -> Result<String, Error> {
    actor
        .send(command)
        .await
        .map_err(Error::Actor)?
        .map_err(|e| match e {
            e @ rcon::RconError::Read(_) | e @ rcon::RconError::Write(_) => {
                Error::BrokenConnection(e)
            }
            e @ rcon::RconError::Connect(_) => Error::Connect(e),
            e @ rcon::RconError::AuthFail => Error::Authenticate(e),
            e => Error::Command(e),
        })
}
