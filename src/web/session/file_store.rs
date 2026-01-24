use serde::de;
use std::{io, path};
use tokio::{
    fs,
    sync::{mpsc, oneshot},
};
use tokio_util::sync;

enum Message<T> {
    Load(oneshot::Sender<Option<T>>),
    Save(T),
}

#[derive(Clone)]
pub struct FileStore<T> {
    sender: mpsc::UnboundedSender<Message<T>>,
    complete: sync::CancellationToken,
}

impl<T> FileStore<T>
where
    T: serde::Serialize + de::DeserializeOwned + Default + Send + 'static,
{
    pub fn new<P: AsRef<path::Path>>(path: P, cancel: sync::CancellationToken) -> Self {
        let complete = sync::CancellationToken::new();
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(handler(
            path.as_ref().to_path_buf(),
            receiver,
            cancel.clone(),
            complete.clone(),
        ));

        Self { sender, complete }
    }

    pub async fn load(&self) -> T {
        let (tx, rx) = oneshot::channel();

        if let Err(err) = self.sender.send(Message::Load(tx)) {
            tracing::warn!(
                %err, "Failed to send the message to the file store task",
            );

            return T::default();
        }

        rx.await
            .inspect_err(|err| {
                tracing::warn!(
                    %err, "The file store task closed the channel before responding",
                )
            })
            .unwrap_or_default()
            .unwrap_or_default()
    }

    pub async fn save(&self, state: T) {
        if self.sender.send(Message::Save(state)).is_err() {
            tracing::warn!("Failed to send the session state to the file store task");
        }
    }

    pub fn shutdown(self) -> sync::WaitForCancellationFutureOwned {
        self.complete.cancelled_owned()
    }
}

async fn handler<T>(
    path: path::PathBuf,
    mut receiver: mpsc::UnboundedReceiver<Message<T>>,
    cancel: sync::CancellationToken,
    complete: sync::CancellationToken,
) where
    T: serde::Serialize + de::DeserializeOwned + Default,
{
    let _guard = cancel.drop_guard_ref();
    let _guard = complete.drop_guard();

    loop {
        match receiver.recv().await {
            Some(Message::Load(responder)) => load_state(&path, responder).await,
            Some(Message::Save(state)) => save_state(&path, state).await,
            None => break,
        }
    }

    tracing::info!("All senders were closed, shutting down.");
}

async fn load_state<T>(path: &path::Path, responder: oneshot::Sender<Option<T>>)
where
    T: de::DeserializeOwned + Default,
{
    let file_data = fs::read_to_string(&path)
        .await
        .inspect_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                tracing::warn!(
                    path = %path.display(), %err, "Failed to read the session state from file",
                );
            }
        })
        .unwrap_or_default();

    let file_state = serde_yaml_ng::from_str(&file_data)
        .inspect_err(|err| {
            tracing::warn!(
                path = %path.display(), %err, "Failed to deserialize session data from file",
            )
        })
        .unwrap_or_default();

    if responder.send(Some(file_state)).is_err() {
        tracing::warn!("Failed to send the session state response to the caller");
    }
}

async fn save_state<T>(path: &path::Path, state: T)
where
    T: serde::Serialize,
{
    match serde_yaml_ng::to_string(&state) {
        Ok(state) => {
            if let Err(err) = fs::write(path, state).await {
                tracing::warn!(path = %path.display(), %err, "Failed to save the session state into the file");
            }
        }
        Err(err) => tracing::warn!(%err, "Failed to serialize the session state"),
    }
}
