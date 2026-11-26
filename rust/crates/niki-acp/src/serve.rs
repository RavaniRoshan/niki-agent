//! The read loop over one transport, and the stdio entry point.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use niki_session::SessionStore;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::task::JoinSet;

use crate::rpc::{self, Connection, Incoming, RpcError};
use crate::server::{AgentFactory, Server, ServerOptions};

/// Why the protocol stream ended.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("the editor connection failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Serve the protocol over `reader` and `writer`.
///
/// Each frame is newline-delimited JSON. Requests are answered concurrently so
/// a cancel or a permission answer is handled while a prompt is in flight.
///
/// # Errors
///
/// Returns [`ServerError::Io`] when the transport fails; end of input is a
/// normal shutdown.
pub async fn serve<R, W>(
    reader: R,
    writer: W,
    factory: AgentFactory,
    options: ServerOptions,
) -> Result<(), ServerError>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let store = options.store.as_deref().and_then(open_store);
    let (connection, frames) = Connection::channel();
    let writer_task = tokio::spawn(rpc::write_frames(writer, frames));
    let server = Arc::new(Server::new(
        Arc::from(factory),
        connection.clone(),
        store,
        options.default_mode,
    ));

    let mut requests = JoinSet::new();
    let mut reader = BufReader::new(reader);
    while let Some(frame) = rpc::read_frame(&mut reader).await? {
        match rpc::classify(frame) {
            Incoming::Request { id, method, params } => {
                let server = Arc::clone(&server);
                requests.spawn(async move {
                    let outcome = match method {
                        Some(method) => server.route(&method, params).await,
                        None => Err(RpcError::new(
                            rpc::INVALID_REQUEST,
                            "Invalid request: method must be a string",
                        )),
                    };
                    server.respond(id, outcome);
                });
            }
            Incoming::Notification { method, params } => {
                server.notification(&method, params).await;
            }
            Incoming::Response { id, outcome } => connection.resolve(&id, outcome),
            Incoming::Ignore => {}
        }
    }

    // A client that closed its side cannot answer a permission prompt, so the
    // turns still waiting for one are released, and the answers already being
    // produced are flushed before the writer stops. A turn that never returns
    // cannot hold the process open.
    connection.shutdown();
    let draining = async { while requests.join_next().await.is_some() {} };
    if tokio::time::timeout(Duration::from_secs(5), draining)
        .await
        .is_err()
    {
        requests.abort_all();
    }
    drop(server);
    drop(connection);
    match writer_task.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(ServerError::Io(error)),
        Err(error) => Err(ServerError::Io(std::io::Error::other(error))),
    }
}

/// Serve the protocol on this process's stdin and stdout.
///
/// Nothing else may write to stdout while this runs: the protocol owns it.
///
/// # Errors
///
/// See [`serve`].
pub async fn serve_stdio(factory: AgentFactory, options: ServerOptions) -> Result<(), ServerError> {
    serve(tokio::io::stdin(), tokio::io::stdout(), factory, options).await
}

/// Open the session store, treating a failure as "sessions are not saved".
fn open_store(path: &Path) -> Option<Arc<Mutex<SessionStore>>> {
    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            eprintln!("niki-rs: warning: sessions are not saved: {error}");
            return None;
        }
    }
    match SessionStore::open(path) {
        Ok(store) => Some(Arc::new(Mutex::new(store))),
        Err(error) => {
            eprintln!("niki-rs: warning: sessions are not saved: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_that_cannot_be_opened_is_a_warning_not_a_failure() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("not-a-file");
        std::fs::create_dir(&path).expect("directory in place of a database");
        assert!(open_store(&path).is_none());
        assert!(open_store(&directory.path().join("sessions.db")).is_some());
    }
}
