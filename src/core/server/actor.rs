use super::rcon;
use std::net;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync;

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

pub struct RconMessage {
    result: oneshot::Sender<Result<String, rcon::RconError>>,
    command: Command,
}

impl RconMessage {
    pub fn new(result: oneshot::Sender<Result<String, rcon::RconError>>, command: Command) -> Self {
        Self { result, command }
    }
}

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

    pub fn start(self, cancel: sync::CancellationToken) -> mpsc::UnboundedSender<RconMessage> {
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(self.handle(receiver, cancel));

        sender
    }

    async fn handle(
        mut self,
        mut chan: mpsc::UnboundedReceiver<RconMessage>,
        cancel_token: sync::CancellationToken,
    ) {
        let _drop_guard = cancel_token.drop_guard();

        while let Some(msg) = chan.recv().await {
            if let Err(e) = msg.result.send(self.handle_message(msg.command).await) {
                tracing::error!(error=?e, "Failed to send the response to the caller");
            }
        }
    }

    async fn handle_message(&mut self, cmd: Command) -> Result<String, rcon::RconError> {
        let mut client = match self.client.take() {
            Some(client) => client,
            None => {
                rcon::RconClient::new()
                    .connect(&self.addr)
                    .await?
                    .authenticate(&self.password)
                    .await?
            }
        };

        let (msg, should_shutdown) = match cmd {
            Command::Stop => (cmd.into(), true),
            _ => (cmd.into(), false),
        };

        match client.command(msg).await {
            Ok(res) => {
                if should_shutdown {
                    let _ = client.disconnect().await;
                } else {
                    self.client.replace(client);
                }

                Ok(res)
            }
            err => {
                let _ = client.disconnect().await;

                err
            }
        }
    }
}
