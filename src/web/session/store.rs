use actix::{Actor, AsyncContext};
use actix_session::storage;
use rand::distr::{self, SampleString};
use std::{collections, time};

type SessionState = collections::HashMap<String, String>;

#[derive(Debug)]
struct SessionEntry {
    ttl: time::Duration,
    timer: time::Instant,
    state: SessionState,
}

impl SessionEntry {
    fn new(ttl: time::Duration, state: SessionState) -> Self {
        let timer = time::Instant::now();

        Self { state, ttl, timer }
    }

    fn is_fresh(&self) -> bool {
        self.timer.elapsed() < self.ttl
    }

    fn update_ttl(&mut self, ttl: time::Duration) {
        self.timer = time::Instant::now();
        self.ttl = ttl;
    }
}

struct LoadMessage(String);

impl actix::Message for LoadMessage {
    type Result = Option<SessionState>;
}

struct SaveMessage {
    state: SessionState,
    ttl: time::Duration,
}

impl actix::Message for SaveMessage {
    type Result = String;
}

struct UpdateMessage {
    key: String,
    state: SessionState,
    ttl: time::Duration,
}

impl actix::Message for UpdateMessage {
    type Result = String;
}

struct UpdateTtlMessage {
    key: String,
    ttl: time::Duration,
}

impl actix::Message for UpdateTtlMessage {
    type Result = ();
}

struct DeleteMessage(String);

impl actix::Message for DeleteMessage {
    type Result = ();
}

#[derive(Default)]
pub struct SessionActor(collections::HashMap<String, SessionEntry>);

impl actix::Actor for SessionActor {
    type Context = actix::Context<Self>;
}

impl actix::Handler<LoadMessage> for SessionActor {
    type Result = <LoadMessage as actix::Message>::Result;

    fn handle(&mut self, msg: LoadMessage, ctx: &mut Self::Context) -> Self::Result {
        match self.0.get(&msg.0) {
            Some(entry) if entry.is_fresh() => Some(entry.state.to_owned()),
            Some(_) => {
                ctx.notify(DeleteMessage(msg.0));

                None
            }
            _ => None,
        }
    }
}

impl actix::Handler<DeleteMessage> for SessionActor {
    type Result = <DeleteMessage as actix::Message>::Result;

    fn handle(&mut self, msg: DeleteMessage, _: &mut Self::Context) -> Self::Result {
        self.0.remove(&msg.0);
    }
}

impl actix::Handler<UpdateMessage> for SessionActor {
    type Result = <UpdateMessage as actix::Message>::Result;

    fn handle(&mut self, msg: UpdateMessage, _: &mut Self::Context) -> Self::Result {
        self.0
            .insert(msg.key.clone(), SessionEntry::new(msg.ttl, msg.state));

        msg.key
    }
}

impl actix::Handler<SaveMessage> for SessionActor {
    type Result = <SaveMessage as actix::Message>::Result;

    fn handle(&mut self, msg: SaveMessage, _: &mut Self::Context) -> Self::Result {
        let mut rng = rand::rng();
        let key = distr::Alphanumeric.sample_string(&mut rng, 32);

        self.0
            .insert(key.clone(), SessionEntry::new(msg.ttl, msg.state));

        key
    }
}

impl actix::Handler<UpdateTtlMessage> for SessionActor {
    type Result = <UpdateTtlMessage as actix::Message>::Result;

    fn handle(&mut self, msg: UpdateTtlMessage, _: &mut Self::Context) -> Self::Result {
        self.0.entry(msg.key).and_modify(|e| e.update_ttl(msg.ttl));
    }
}

#[derive(Clone)]
pub struct SessionStore {
    addr: actix::Addr<SessionActor>,
}

impl Default for SessionStore {
    fn default() -> Self {
        let actor = SessionActor::default();
        let addr = actor.start();

        Self { addr }
    }
}

impl storage::SessionStore for SessionStore {
    async fn load(
        &self,
        session_key: &storage::SessionKey,
    ) -> Result<Option<SessionState>, storage::LoadError> {
        self.addr
            .send(LoadMessage(session_key.as_ref().to_owned()))
            .await
            .map_err(|err| storage::LoadError::Other(err.into()))
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<storage::SessionKey, storage::SaveError> {
        self.addr
            .send(SaveMessage {
                state: session_state,
                ttl: ttl.unsigned_abs(),
            })
            .await
            .map_err(|err| storage::SaveError::Other(err.into()))?
            .try_into()
            .map_err(|err: <storage::SessionKey as TryFrom<String>>::Error| {
                storage::SaveError::Other(err.into())
            })
    }

    async fn update(
        &self,
        session_key: storage::SessionKey,
        session_state: SessionState,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<storage::SessionKey, storage::UpdateError> {
        self.addr
            .send(UpdateMessage {
                key: session_key.into(),
                state: session_state,
                ttl: ttl.unsigned_abs(),
            })
            .await
            .map_err(|err| storage::UpdateError::Other(err.into()))?
            .try_into()
            .map_err(|err: <storage::SessionKey as TryFrom<String>>::Error| {
                storage::UpdateError::Other(err.into())
            })
    }

    async fn update_ttl(
        &self,
        session_key: &storage::SessionKey,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<(), anyhow::Error> {
        Ok(self
            .addr
            .send(UpdateTtlMessage {
                key: session_key.as_ref().into(),
                ttl: ttl.unsigned_abs(),
            })
            .await?)
    }

    async fn delete(&self, session_key: &storage::SessionKey) -> Result<(), anyhow::Error> {
        Ok(self
            .addr
            .send(DeleteMessage(session_key.as_ref().into()))
            .await?)
    }
}
