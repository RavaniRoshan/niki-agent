//! SQLite session store: transcripts, resume, and checkpoints.
//!
//! The store owns `~/.niki/sessions.db` (ADR-006). It reads and writes only its
//! own schema; a Python session database is not imported in W1–W5. Timestamps
//! are Unix seconds taken from [`std::time::SystemTime`], and message order is
//! the autoincrement id, so a transcript replays in the order it was written.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    title TEXT,
    cwd TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS messages_session ON messages(session_id, id);
CREATE TABLE IF NOT EXISTS checkpoints (
    session_id TEXT NOT NULL REFERENCES sessions(id),
    label TEXT NOT NULL,
    message_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, label)
);
";

/// Opaque session identifier, generated at [`SessionStore::create`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(String);

impl SessionId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One persisted message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: i64,
    pub session_id: SessionId,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

/// A session as the `--resume` list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub id: SessionId,
    /// Message count, so the list can show an empty session for what it is.
    pub messages: i64,
    /// The first prompt the person typed, when the session has one.
    pub first_prompt: Option<String>,
    pub title: Option<String>,
    pub cwd: String,
    pub created_at: i64,
}

/// An open session database.
#[derive(Debug)]
pub struct SessionStore {
    connection: Connection,
}

/// The columns every [`Summary`] query reads, in the order its mapper expects.
///
/// The first prompt is the oldest stored `user` message; the count includes
/// every role. Both subqueries read the existing `messages` table, so no
/// schema change is needed.
const SUMMARY_COLUMNS: &str = "sessions.id, sessions.title, sessions.cwd, sessions.created_at,
     (SELECT COUNT(*) FROM messages WHERE messages.session_id = sessions.id),
     (SELECT content FROM messages
      WHERE messages.session_id = sessions.id AND messages.role = 'user'
      ORDER BY messages.id ASC LIMIT 1)";

impl SessionStore {
    /// Open or create the database at `path` and apply the schema.
    ///
    /// The parent directory must exist; creating it is the caller's job.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the file cannot be opened or the
    /// schema cannot be applied.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        let path = path.as_ref();
        let connection = Connection::open(path)?;
        // A transcript holds prompts, answers and tool output, so the file
        // must not be readable by another account on the machine.
        restrict_permissions(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.execute_batch(SCHEMA)?;
        Ok(Self { connection })
    }

    /// Create a session rooted at `cwd` and return its id.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the insert fails, and
    /// [`SessionError::NonUtf8Path`] when `cwd` is not UTF-8.
    pub fn create(&self, cwd: impl AsRef<Path>) -> Result<SessionId, SessionError> {
        let cwd = cwd.as_ref().to_str().ok_or(SessionError::NonUtf8Path)?;
        let id = SessionId(self.next_id()?);
        self.connection.execute(
            "INSERT INTO sessions (id, created_at, title, cwd) VALUES (?1, ?2, NULL, ?3)",
            params![id.as_str(), now()?, cwd],
        )?;
        Ok(id)
    }

    /// Every session, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the query fails.
    pub fn list(&self) -> Result<Vec<Summary>, SessionError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SUMMARY_COLUMNS}
             FROM sessions
             ORDER BY sessions.created_at DESC, sessions.rowid DESC",
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(Summary {
                id: SessionId(row.get(0)?),
                title: row.get(1)?,
                cwd: row.get(2)?,
                created_at: row.get(3)?,
                messages: row.get(4)?,
                first_prompt: row.get(5)?,
            })
        })?;
        let mut summaries = Vec::new();
        for summary in rows {
            summaries.push(summary?);
        }
        Ok(summaries)
    }

    /// One session by id, when it exists.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the query fails.
    pub fn find(&self, id: &str) -> Result<Option<Summary>, SessionError> {
        let summary = self
            .connection
            .query_row(
                &format!("SELECT {SUMMARY_COLUMNS} FROM sessions WHERE sessions.id = ?1"),
                params![id],
                |row| {
                    Ok(Summary {
                        id: SessionId(row.get(0)?),
                        title: row.get(1)?,
                        cwd: row.get(2)?,
                        created_at: row.get(3)?,
                        messages: row.get(4)?,
                        first_prompt: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(summary)
    }

    /// The newest session rooted at `cwd`, when there is one.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the query fails.
    pub fn latest_for(&self, cwd: impl AsRef<Path>) -> Result<Option<Summary>, SessionError> {
        let cwd = cwd.as_ref().to_str().ok_or(SessionError::NonUtf8Path)?;
        let summary = self
            .connection
            .query_row(
                &format!(
                    "SELECT {SUMMARY_COLUMNS}
                     FROM sessions WHERE sessions.cwd = ?1
                     ORDER BY sessions.created_at DESC, sessions.rowid DESC
                     LIMIT 1"
                ),
                params![cwd],
                |row| {
                    Ok(Summary {
                        id: SessionId(row.get(0)?),
                        title: row.get(1)?,
                        cwd: row.get(2)?,
                        created_at: row.get(3)?,
                        messages: row.get(4)?,
                        first_prompt: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(summary)
    }

    /// Append one message and return its id.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the insert fails.
    pub fn append(
        &self,
        session: &SessionId,
        role: &str,
        content: &str,
    ) -> Result<i64, SessionError> {
        self.ensure_session(session)?;
        self.connection.execute(
            "INSERT INTO messages (session_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![session.as_str(), role, content, now()?],
        )?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Every message of a session, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the query fails.
    pub fn load(&self, session: &SessionId) -> Result<Vec<Message>, SessionError> {
        self.ensure_session(session)?;
        self.messages(session, None)
    }

    /// Record a checkpoint after the latest message and return its watermark.
    ///
    /// Re-using a label moves that checkpoint forward to the current end of the
    /// transcript, which is what a manual checkpoint should do.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the write fails.
    pub fn checkpoint(&self, session: &SessionId, label: &str) -> Result<i64, SessionError> {
        self.ensure_session(session)?;
        let watermark: i64 = self.connection.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM messages WHERE session_id = ?1",
            params![session.as_str()],
            |row| row.get(0),
        )?;
        self.connection.execute(
            "INSERT INTO checkpoints (session_id, label, message_id, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id, label)
             DO UPDATE SET message_id = excluded.message_id, created_at = excluded.created_at",
            params![session.as_str(), label, watermark, now()?],
        )?;
        Ok(watermark)
    }

    /// Messages written up to a checkpoint, oldest first.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, [`SessionError::UnknownCheckpoint`] for a label the session never
    /// recorded, and [`SessionError::Sqlite`] when the query fails.
    pub fn resume(&self, session: &SessionId, label: &str) -> Result<Vec<Message>, SessionError> {
        self.ensure_session(session)?;
        let watermark: Option<i64> = self
            .connection
            .query_row(
                "SELECT message_id FROM checkpoints WHERE session_id = ?1 AND label = ?2",
                params![session.as_str(), label],
                |row| row.get(0),
            )
            .optional()?;
        let watermark = watermark.ok_or_else(|| SessionError::UnknownCheckpoint {
            session: session.clone(),
            label: label.to_owned(),
        })?;
        self.messages(session, Some(watermark))
    }

    fn messages(
        &self,
        session: &SessionId,
        watermark: Option<i64>,
    ) -> Result<Vec<Message>, SessionError> {
        let mut statement = self.connection.prepare(
            "SELECT id, session_id, role, content, created_at
             FROM messages
             WHERE session_id = ?1 AND (?2 IS NULL OR id <= ?2)
             ORDER BY id ASC",
        )?;
        let rows = statement.query_map(params![session.as_str(), watermark], |row| {
            Ok(Message {
                id: row.get(0)?,
                session_id: SessionId(row.get(1)?),
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut messages = Vec::new();
        for message in rows {
            messages.push(message?);
        }
        Ok(messages)
    }

    fn ensure_session(&self, session: &SessionId) -> Result<(), SessionError> {
        let known: Option<i64> = self
            .connection
            .query_row(
                "SELECT rowid FROM sessions WHERE id = ?1",
                params![session.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        known.map_or_else(
            || {
                Err(SessionError::UnknownSession {
                    id: session.clone(),
                })
            },
            |_| Ok(()),
        )
    }

    fn next_id(&self) -> Result<String, SessionError> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionError::Clock)?
            .as_nanos();
        for attempt in 0..16_u32 {
            let id = format!("{nanos:032x}-{attempt:02x}");
            let taken: Option<i64> = self
                .connection
                .query_row(
                    "SELECT rowid FROM sessions WHERE id = ?1",
                    params![id.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            if taken.is_none() {
                return Ok(id);
            }
        }
        Err(SessionError::IdCollision)
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<(), SessionError> {
    use std::os::unix::fs::PermissionsExt;

    if path.as_os_str() == ":memory:" {
        return Ok(());
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|source| {
        SessionError::Permissions {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<(), SessionError> {
    Ok(())
}

fn now() -> Result<i64, SessionError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SessionError::Clock)?
        .as_secs();
    i64::try_from(seconds).map_err(|_| SessionError::Clock)
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session store: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("unknown session: {id}")]
    UnknownSession { id: SessionId },
    #[error("session {session} has no checkpoint named `{label}`")]
    UnknownCheckpoint { session: SessionId, label: String },
    #[error("the working directory is not valid UTF-8")]
    NonUtf8Path,
    #[error("cannot restrict the permissions of {path}: {source}")]
    Permissions {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the system clock is before the Unix epoch")]
    Clock,
    #[error("could not allocate a unique session id")]
    IdCollision,
}
