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
    #[error("Failed to parse server tick stats: {0}")]
    TickStats(String),
}

#[derive(Clone)]
pub struct Client(actix::Addr<actor::RconActor>);

#[derive(serde::Serialize)]
pub struct TickStats {
    pub average: String,
    pub target: String,
    pub p50: String,
    pub p95: String,
    pub p99: String,
}

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

    pub async fn query_tick(&self) -> Result<TickStats, Error> {
        let tick_stats =
            run_command(&self.0, actor::Command::Other("tick query".to_string())).await?;

        // Example server output:
        // Target tick rate: 20.0 per second.
        // Average time per tick: 13.2ms (Target: 50.0ms)
        // Percentiles: P50: 13.0ms P95: 16.0ms P99: 18.6ms, sample: 100
        let tick_stats_stripped = tick_stats.replace([':', ',', '(', ')'], " ");
        let timings: Vec<_> = tick_stats_stripped
            .split_whitespace()
            .filter(|w| w.ends_with("ms"))
            .collect();

        if timings.len() != 5 {
            Err(Error::TickStats(tick_stats))
        } else {
            Ok(TickStats {
                average: timings[0].to_string(),
                target: timings[1].to_string(),
                p50: timings[2].to_string(),
                p95: timings[3].to_string(),
                p99: timings[4].to_string(),
            })
        }
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
