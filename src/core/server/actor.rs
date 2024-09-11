use super::rcon;
use std::net;

pub struct RconActor {
    addr: net::SocketAddr,
    password: secrecy::SecretString,
    client: Option<rcon::RconClient<rcon::Authenticated>>,
}

impl RconActor {
    pub fn new(addr: net::SocketAddr, password: secrecy::SecretString) -> Self {
        Self {
            addr,
            password,
            client: None,
        }
    }
}

impl actix::Actor for RconActor {
    type Context = actix::Context<Self>;
}

pub enum Command {
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
