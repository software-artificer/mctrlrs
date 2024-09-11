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

struct Command(String);

impl actix::Message for Command {
    type Result = Result<String, rcon::RconError>;
}

impl actix::Handler<Command> for RconActor {
    type Result = <Command as actix::Message>::Result;

    fn handle(&mut self, msg: Command, _: &mut Self::Context) -> Self::Result {
        if self.client.is_none() {
            self.client = Some(
                rcon::RconClient::new()
                    .connect(self.addr)?
                    .authenticate(self.password.clone())?,
            );
        }

        let mut client = self.client.take().unwrap();

        match client.command(msg.0) {
            Ok(res) => {
                self.client = Some(client);

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
        run_command(&self.0, "save-all", Error::SaveAll).await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Error> {
        run_command(&self.0, "stop", Error::Stop).await?;

        Ok(())
    }
}

async fn run_command<F, C>(
    actor: &actix::Addr<RconActor>,
    command: C,
    err_func: F,
) -> Result<String, Error>
where
    C: AsRef<str>,
    F: FnOnce(rcon::RconError) -> Error,
{
    actor
        .send(Command(command.as_ref().to_string()))
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
