mod rcon;

use actix::Actor;
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
    #[error("Failed to send a message to the actor: {0}")]
    Actor(#[source] actix::MailboxError),
}

struct RconActor {
    addr: net::SocketAddr,
    password: secrecy::SecretString,
    client: Option<rcon::RconClient<rcon::Authenticated>>,
}

impl actix::Actor for RconActor {
    type Context = actix::Context<Self>;
}

enum Command {
    Stop,
    Other(String),
}

impl From<Command> for String {
    fn from(value: Command) -> Self {
        match value {
            Command::Stop => "stop".to_string(),
            Command::Other(cmd) => cmd,
        }
    }
}

impl actix::Message for Command {
    type Result = Result<String, rcon::RconError>;
}

impl actix::Handler<Command> for RconActor {
    type Result = <Command as actix::Message>::Result;

    fn handle(&mut self, msg: Command, _: &mut Self::Context) -> Self::Result {
        let mut client = match self.client.take() {
            Some(client) => client,
            None => rcon::RconClient::new()
                .connect(self.addr)?
                .authenticate(self.password.clone())?,
        };

        let (msg, should_shutdown) = match msg {
            Command::Stop => (msg.into(), true),
            _ => (msg.into(), false),
        };

        match client.command(msg) {
            Ok(res) => {
                if should_shutdown {
                    let _ = client.disconnect();
                } else {
                    self.client = Some(client);
                }

                Ok(res)
            }
            err => {
                let _ = client.disconnect();

                err
            }
        }
    }
}

#[derive(Clone)]
pub struct Client(actix::Addr<RconActor>);

impl Client {
    pub fn new(addr: net::SocketAddr, password: secrecy::SecretString) -> Self {
        let actor = RconActor {
            addr,
            password,
            client: None,
        };

        Self(actor.start())
    }

    pub async fn save_all(&self) -> Result<(), Error> {
        run_command(
            &self.0,
            Command::Other("save-all".to_string()),
            Error::SaveAll,
        )
        .await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Error> {
        run_command(&self.0, Command::Stop, Error::Stop).await?;

        Ok(())
    }
}

async fn run_command<F>(
    actor: &actix::Addr<RconActor>,
    command: Command,
    err_func: F,
) -> Result<String, Error>
where
    F: FnOnce(rcon::RconError) -> Error,
{
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
            e => err_func(e),
        })
}
