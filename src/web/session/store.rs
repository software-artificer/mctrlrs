use actix_session::storage;
use anyhow::Context;
use rand::distr::{self, SampleString};
use std::{collections, time};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync;

type SessionState = collections::HashMap<String, SessionEntry>;

type SessionData = collections::HashMap<String, String>;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct SessionEntry {
    ttl: time::Duration,
    timer: time::SystemTime,
    state: SessionData,
}

impl SessionEntry {
    fn new(ttl: time::Duration, state: collections::HashMap<String, String>) -> Self {
        let timer = time::SystemTime::now();

        Self { state, ttl, timer }
    }

    fn is_fresh(&self) -> bool {
        self.timer
            .elapsed()
            .map(|dur| dur < self.ttl)
            .unwrap_or_default()
    }

    fn update_ttl(&mut self, ttl: time::Duration) {
        self.timer = time::SystemTime::now();
        self.ttl = ttl;
    }
}

enum Message {
    Load {
        result: oneshot::Sender<Option<SessionData>>,
        key: String,
    },
    Save {
        result: oneshot::Sender<()>,
        key: String,
        state: SessionData,
        ttl: time::Duration,
    },
    Update {
        result: oneshot::Sender<()>,
        key: String,
        state: SessionData,
        ttl: time::Duration,
    },
    UpdateTtl {
        result: oneshot::Sender<()>,
        key: String,
        ttl: time::Duration,
    },
    Delete {
        result: oneshot::Sender<()>,
        key: String,
    },
}

async fn session_handler(
    file_store: super::FileStore<SessionState>,
    mut receiver: mpsc::UnboundedReceiver<Message>,
    cancel: sync::CancellationToken,
    complete: sync::CancellationToken,
) {
    let _cancel_guard = cancel.drop_guard();
    let _complete_guard = complete.drop_guard();

    let mut store = file_store.load().await;

    while let Some(message) = receiver.recv().await {
        match message {
            Message::Load { result, key } => {
                if let Err(e) = result.send(match store.get(&key) {
                    Some(state) => {
                        if state.is_fresh() {
                            Some(state.state.clone())
                        } else {
                            None
                        }
                    }
                    None => None,
                }) {
                    tracing::warn!(error=?e, "Tried to send the response to the closed channel.");
                }
            }
            Message::Save {
                result,
                key,
                state,
                ttl,
            } => {
                store.insert(key, SessionEntry::new(ttl, state));

                if let Err(e) = result.send(()) {
                    tracing::warn!(error=?e, "Tried to send the response to the closed channel.");
                }
            }
            Message::Update {
                result,
                key,
                state,
                ttl,
            } => {
                store.insert(key, SessionEntry::new(ttl, state));

                if let Err(e) = result.send(()) {
                    tracing::warn!(error=?e, "Tried to send the response to the closed channel.");
                }
            }
            Message::UpdateTtl { result, key, ttl } => {
                store.entry(key).and_modify(|v| v.update_ttl(ttl));

                if let Err(e) = result.send(()) {
                    tracing::warn!(error=?e, "Tried to send the response to the closed channel.");
                }
            }
            Message::Delete { result, key } => {
                store.remove(&key);

                if let Err(e) = result.send(()) {
                    tracing::warn!(error=?e, "Tried to send the response to the closed channel.");
                }
            }
        }
    }

    file_store.save(store).await;
    file_store.shutdown().await;
}

#[derive(Clone)]
pub struct SessionStore {
    sender: mpsc::UnboundedSender<Message>,
    complete: sync::CancellationToken,
}

impl SessionStore {
    pub fn new(fs: super::FileStore<SessionState>, cancel: sync::CancellationToken) -> Self {
        let complete = sync::CancellationToken::new();
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(session_handler(fs, receiver, cancel, complete.clone()));

        Self { sender, complete }
    }

    pub fn shutdown(self) -> sync::WaitForCancellationFutureOwned {
        self.complete.cancelled_owned()
    }
}

impl storage::SessionStore for SessionStore {
    async fn load(
        &self,
        session_key: &storage::SessionKey,
    ) -> Result<Option<SessionData>, storage::LoadError> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(Message::Load {
                result: sender,
                key: session_key.as_ref().to_owned(),
            })
            .map_err(|err| storage::LoadError::Other(err.into()))?;

        receiver
            .await
            .context("Failed to load the session state")
            .map_err(storage::LoadError::Other)
    }

    async fn save(
        &self,
        state: SessionData,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<storage::SessionKey, storage::SaveError> {
        let mut rng = rand::rng();
        let key = distr::Alphanumeric.sample_string(&mut rng, 32);

        let session_key = storage::SessionKey::try_from(key.clone())
            .context("Failed to convert String to SessionKey")
            .map_err(storage::SaveError::Other)?;

        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(Message::Save {
                result: sender,
                key: key.clone(),
                state,
                ttl: ttl.unsigned_abs(),
            })
            .map_err(|err| storage::SaveError::Other(err.into()))?;

        receiver
            .await
            .context("Failed to save the session state")
            .map_err(storage::SaveError::Other)?;

        Ok(session_key)
    }

    async fn update(
        &self,
        session_key: storage::SessionKey,
        session_state: SessionData,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<storage::SessionKey, storage::UpdateError> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(Message::Update {
                result: sender,
                key: session_key.as_ref().to_string(),
                state: session_state,
                ttl: ttl.unsigned_abs(),
            })
            .map_err(|err| storage::UpdateError::Other(err.into()))?;

        receiver
            .await
            .context("Failed to update the session state")
            .map_err(storage::UpdateError::Other)?;

        Ok(session_key)
    }

    async fn update_ttl(
        &self,
        session_key: &storage::SessionKey,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<(), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(Message::UpdateTtl {
                result: sender,
                key: session_key.as_ref().into(),
                ttl: ttl.unsigned_abs(),
            })
            .context("Failed to update the session TTL")?;

        receiver.await.context("Failed to update the session TTL")
    }

    async fn delete(&self, session_key: &storage::SessionKey) -> Result<(), anyhow::Error> {
        let (sender, receiver) = oneshot::channel();

        self.sender
            .send(Message::Delete {
                result: sender,
                key: session_key.as_ref().into(),
            })
            .context("Failed to delete the session key")?;

        receiver.await.context("Failed to delete the session")
    }
}
