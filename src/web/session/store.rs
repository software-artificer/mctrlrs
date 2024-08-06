use actix_session::storage;
use rand::distributions::{self, DistString};
use std::{collections, time};
use tokio::sync::{mpsc, oneshot};

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

enum Message {
    Load(String, oneshot::Sender<Option<SessionState>>),
    Save(SessionState, time::Duration, oneshot::Sender<String>),
    Update(
        String,
        SessionState,
        time::Duration,
        oneshot::Sender<String>,
    ),
    UpdateTtl(String, time::Duration),
    Delete(String),
}

#[derive(Clone)]
pub struct SessionStore {
    channel: mpsc::UnboundedSender<Message>,
}

impl Default for SessionStore {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        start_store(tx.clone(), rx);

        Self { channel: tx }
    }
}

fn start_store(tx_self: mpsc::UnboundedSender<Message>, mut rx: mpsc::UnboundedReceiver<Message>) {
    tokio::spawn(async move {
        let mut store: collections::HashMap<String, SessionEntry> = Default::default();

        while let Some(message) = rx.recv().await {
            match message {
                Message::Load(key, tx) => {
                    let entry = match store.get(&key) {
                        Some(entry) if entry.is_fresh() => Some(entry.state.to_owned()),
                        Some(_) => {
                            let _ = tx_self.send(Message::Delete(key));

                            None
                        }
                        _ => None,
                    };

                    let _ = tx.send(entry);
                }
                Message::Save(state, ttl, tx) => {
                    let mut rng = rand::thread_rng();
                    let key = distributions::Alphanumeric.sample_string(&mut rng, 32);

                    store.insert(key.clone(), SessionEntry::new(ttl, state));

                    let _ = tx.send(key);
                }
                Message::Update(key, state, ttl, tx) => {
                    store.insert(key.clone(), SessionEntry::new(ttl, state));

                    let _ = tx.send(key);
                }
                Message::UpdateTtl(key, ttl) => {
                    store.entry(key).and_modify(|e| e.update_ttl(ttl));
                }
                Message::Delete(key) => {
                    store.remove(&key);
                }
            }
        }
    });
}

impl storage::SessionStore for SessionStore {
    async fn load(
        &self,
        session_key: &storage::SessionKey,
    ) -> Result<Option<SessionState>, storage::LoadError> {
        let (tx, rx) = oneshot::channel();

        self.channel
            .send(Message::Load(session_key.as_ref().to_owned(), tx))
            .map_err(|err| storage::LoadError::Other(err.into()))?;

        rx.await.map_err(|e| storage::LoadError::Other(e.into()))
    }

    async fn save(
        &self,
        session_state: SessionState,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<storage::SessionKey, storage::SaveError> {
        let (tx, rx) = oneshot::channel();

        self.channel
            .send(Message::Save(session_state, ttl.unsigned_abs(), tx))
            .map_err(|err| storage::SaveError::Other(err.into()))?;

        rx.await
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
        let (tx, rx) = oneshot::channel();

        self.channel
            .send(Message::Update(
                session_key.into(),
                session_state,
                ttl.unsigned_abs(),
                tx,
            ))
            .map_err(|err| storage::UpdateError::Other(err.into()))?;

        rx.await
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
        self.channel.send(Message::UpdateTtl(
            session_key.as_ref().into(),
            ttl.unsigned_abs(),
        ))?;

        Ok(())
    }

    async fn delete(&self, session_key: &storage::SessionKey) -> Result<(), anyhow::Error> {
        self.channel
            .send(Message::Delete(session_key.as_ref().into()))?;

        Ok(())
    }
}
