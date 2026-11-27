//! SQLite session store: transcripts, resume, and checkpoints.
//!
//! The store owns `~/.niki/sessions.db` (ADR-006). It reads and writes only its
//! own schema and imports no other session store. Timestamps are Unix seconds
//! taken from [`std::time::SystemTime`], and message order is the autoincrement
//! id, so a transcript replays in the order it was written.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, ToSql, params};

pub mod export;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    title TEXT,
    cwd TEXT NOT NULL,
    archived_at INTEGER
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
    /// Whether the session was archived, which hides it from the lists a
    /// person picks a session to resume from.
    pub archived: bool,
    pub created_at: i64,
    /// The latest stored write to the session, in Unix seconds.
    ///
    /// The schema records no separate update column, so this is the newest of
    /// the session's `created_at` and its messages' `created_at`.
    pub updated_at: i64,
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
/// schema change is needed. The update time is the newest of the session's
/// creation and its latest message; `MAX` with two arguments is SQLite's
/// scalar maximum, not the aggregate. The archived flag is set when the
/// session carries an archival stamp.
const SUMMARY_COLUMNS: &str = "sessions.id, sessions.title, sessions.cwd, sessions.created_at,
     MAX(sessions.created_at,
         COALESCE((SELECT MAX(created_at) FROM messages
                   WHERE messages.session_id = sessions.id), sessions.created_at)),
     (SELECT COUNT(*) FROM messages WHERE messages.session_id = sessions.id),
     (SELECT content FROM messages
      WHERE messages.session_id = sessions.id AND messages.role = 'user'
      ORDER BY messages.id ASC LIMIT 1),
     (sessions.archived_at IS NOT NULL)";

fn read_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<Summary> {
    Ok(Summary {
        id: SessionId(row.get(0)?),
        title: row.get(1)?,
        cwd: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        messages: row.get(5)?,
        first_prompt: row.get(6)?,
        archived: row.get(7)?,
    })
}

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
        add_missing_title(&connection)?;
        add_missing_archived(&connection)?;
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

    /// The title a session carries, when it has one.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the query fails.
    pub fn title(&self, session: &SessionId) -> Result<Option<String>, SessionError> {
        self.ensure_session(session)?;
        let title = self.connection.query_row(
            "SELECT title FROM sessions WHERE id = ?1",
            params![session.as_str()],
            |row| row.get(0),
        )?;
        Ok(title)
    }

    /// Replace a session's title and return what was stored.
    ///
    /// The title is trimmed, and an empty one clears the column back to `NULL`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the write fails.
    pub fn set_title(
        &self,
        session: &SessionId,
        title: Option<&str>,
    ) -> Result<Option<String>, SessionError> {
        self.ensure_session(session)?;
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned);
        self.connection.execute(
            "UPDATE sessions SET title = ?2 WHERE id = ?1",
            params![session.as_str(), title],
        )?;
        Ok(title)
    }

    /// Mark a session archived, or restore it, without touching its transcript.
    ///
    /// An archived session keeps its messages and stays readable; the flag is
    /// only what the resume lists filter on. Archiving an archived session
    /// refreshes the stamp, and restoring a live one is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the write fails.
    pub fn set_archived(&self, session: &SessionId, archived: bool) -> Result<(), SessionError> {
        self.ensure_session(session)?;
        let stamp = if archived { Some(now()?) } else { None };
        self.connection.execute(
            "UPDATE sessions SET archived_at = ?2 WHERE id = ?1",
            params![session.as_str(), stamp],
        )?;
        Ok(())
    }

    /// Copy a session's messages into a new session and return the new id.
    ///
    /// The copy keeps the source's workspace, message order, roles, bodies and
    /// write times, so the transcript replays exactly as it was written. The
    /// fork is titled after the source with ` (fork)` appended, and an
    /// untitled source stays untitled. The source is not modified.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::UnknownSession`] for an id this store did not
    /// create, and [`SessionError::Sqlite`] when the copy fails.
    pub fn fork(&self, session: &SessionId) -> Result<SessionId, SessionError> {
        self.ensure_session(session)?;
        let id = SessionId(self.next_id()?);
        let title: Option<String> = self.connection.query_row(
            "SELECT title FROM sessions WHERE id = ?1",
            params![session.as_str()],
            |row| row.get(0),
        )?;
        let title = title
            .map(|title| title.trim().to_owned())
            .filter(|title| !title.is_empty())
            .map(|title| format!("{title} (fork)"));
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO sessions (id, created_at, title, cwd)
             SELECT ?1, ?2, ?3, cwd FROM sessions WHERE id = ?4",
            params![id.as_str(), now()?, title, session.as_str()],
        )?;
        transaction.execute(
            "INSERT INTO messages (session_id, role, content, created_at)
             SELECT ?1, role, content, created_at FROM messages
             WHERE session_id = ?2
             ORDER BY id ASC",
            params![id.as_str(), session.as_str()],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    /// Every session, newest first, archived ones included.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the query fails.
    pub fn list(&self) -> Result<Vec<Summary>, SessionError> {
        self.summaries("", &[])
    }

    /// Every live session, newest first; archived sessions are left out.
    ///
    /// This is the listing a person picks a session to resume from. Archiving
    /// hides a session from it without deleting anything, and [`find`] still
    /// reaches that session by id.
    ///
    /// [`find`]: SessionStore::find
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::Sqlite`] when the query fails.
    pub fn list_active(&self) -> Result<Vec<Summary>, SessionError> {
        self.summaries("WHERE sessions.archived_at IS NULL", &[])
    }

    /// Every session rooted at `cwd`, newest first, archived ones included.
    ///
    /// The workspace key is the stored `cwd` text, the same equality
    /// [`SessionStore::latest_active`] resumes by, so a caller must not hash
    /// or otherwise normalise the path itself.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NonUtf8Path`] when `cwd` is not UTF-8, and
    /// [`SessionError::Sqlite`] when the query fails.
    pub fn list_for(&self, cwd: impl AsRef<Path>) -> Result<Vec<Summary>, SessionError> {
        let cwd = cwd.as_ref().to_str().ok_or(SessionError::NonUtf8Path)?;
        self.summaries("WHERE sessions.cwd = ?1", &[&cwd])
    }

    /// Every live session rooted at `cwd`, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NonUtf8Path`] when `cwd` is not UTF-8, and
    /// [`SessionError::Sqlite`] when the query fails.
    pub fn list_for_active(&self, cwd: impl AsRef<Path>) -> Result<Vec<Summary>, SessionError> {
        let cwd = cwd.as_ref().to_str().ok_or(SessionError::NonUtf8Path)?;
        self.summaries(
            "WHERE sessions.cwd = ?1 AND sessions.archived_at IS NULL",
            &[&cwd],
        )
    }

    /// One session by id, when it exists.
    ///
    /// An archived session is still found by its id, so an explicit resume or
    /// fork keeps working.
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
                read_summary,
            )
            .optional()?;
        Ok(summary)
    }

    /// The newest live session rooted at `cwd`, when there is one.
    ///
    /// An archived session is never the workspace's current session, so
    /// `--continue` and `--resume last` skip it and fall back to the newest
    /// one before it.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NonUtf8Path`] when `cwd` is not UTF-8, and
    /// [`SessionError::Sqlite`] when the query fails.
    pub fn latest_active(&self, cwd: impl AsRef<Path>) -> Result<Option<Summary>, SessionError> {
        let cwd = cwd.as_ref().to_str().ok_or(SessionError::NonUtf8Path)?;
        let summary = self
            .connection
            .query_row(
                &format!(
                    "SELECT {SUMMARY_COLUMNS}
                     FROM sessions WHERE sessions.cwd = ?1 AND sessions.archived_at IS NULL
                     ORDER BY sessions.created_at DESC, sessions.rowid DESC
                     LIMIT 1"
                ),
                params![cwd],
                read_summary,
            )
            .optional()?;
        Ok(summary)
    }

    fn summaries(
        &self,
        filter: &str,
        arguments: &[&dyn ToSql],
    ) -> Result<Vec<Summary>, SessionError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {SUMMARY_COLUMNS}
             FROM sessions
             {filter}
             ORDER BY sessions.created_at DESC, sessions.rowid DESC",
        ))?;
        let rows = statement.query_map(arguments, read_summary)?;
        let mut summaries = Vec::new();
        for summary in rows {
            summaries.push(summary?);
        }
        Ok(summaries)
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

/// Add the `title` column to a database created before it was declared.
///
/// `CREATE TABLE IF NOT EXISTS` leaves an existing table alone, so a store
/// opened over an older file needs the column added explicitly.
fn add_missing_title(connection: &Connection) -> Result<(), SessionError> {
    let present: bool = connection.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM pragma_table_info('sessions') WHERE name = 'title'
         )",
        [],
        |row| row.get(0),
    )?;
    if !present {
        connection.execute("ALTER TABLE sessions ADD COLUMN title TEXT", [])?;
    }
    Ok(())
}

/// Add the `archived_at` column to a database created before it was declared.
///
/// The store shipped through 2.1.x without the column, so `CREATE TABLE IF NOT
/// EXISTS` leaves those files alone; the `ALTER` adds it in place and keeps
/// every row. It is re-runnable, because a store is opened on every command.
fn add_missing_archived(connection: &Connection) -> Result<(), SessionError> {
    let present: bool = connection.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM pragma_table_info('sessions') WHERE name = 'archived_at'
         )",
        [],
        |row| row.get(0),
    )?;
    if !present {
        connection.execute("ALTER TABLE sessions ADD COLUMN archived_at INTEGER", [])?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::{SessionError, SessionStore};

    fn open(temp: &TempDir) -> SessionStore {
        SessionStore::open(temp.path().join("sessions.db")).expect("store")
    }

    fn contents(store: &SessionStore, session: &super::SessionId) -> Vec<(String, String, i64)> {
        store
            .load(session)
            .expect("load")
            .into_iter()
            .map(|message| (message.role, message.content, message.created_at))
            .collect()
    }

    #[test]
    fn fork_copies_the_transcript_and_leaves_the_source_untouched() {
        let temp = TempDir::new().expect("temp");
        let store = open(&temp);
        let cwd = temp.path().join("project");
        let source = store.create(&cwd).expect("create");
        store
            .set_title(&source, Some("port the parser"))
            .expect("title");
        store
            .append(&source, "user", "read lib.rs")
            .expect("append");
        store
            .append(&source, "assistant", "read it")
            .expect("append");
        store.append(&source, "tool", "3 lines").expect("append");
        let before = store.load(&source).expect("load");

        let forked = store.fork(&source).expect("fork");

        assert_ne!(forked, source, "the copy gets its own id");
        assert_eq!(
            store.load(&source).expect("load"),
            before,
            "the source transcript is not rewritten"
        );
        let copied = store.load(&forked).expect("load");
        assert_eq!(
            copied
                .iter()
                .map(|message| (message.role.as_str(), message.content.as_str()))
                .collect::<Vec<_>>(),
            [
                ("user", "read lib.rs"),
                ("assistant", "read it"),
                ("tool", "3 lines")
            ],
            "order, roles and bodies are kept"
        );
        assert!(
            copied.windows(2).all(|pair| pair[0].id < pair[1].id),
            "the copy keeps the write order"
        );
        assert_ne!(copied[0].id, before[0].id, "the copy has new message rows");
        assert!(copied.iter().all(|message| message.session_id == forked));
        assert_eq!(
            copied
                .iter()
                .map(|message| message.created_at)
                .collect::<Vec<_>>(),
            before
                .iter()
                .map(|message| message.created_at)
                .collect::<Vec<_>>(),
            "a copy keeps the recorded write times"
        );

        let summary = store.find(forked.as_str()).expect("find").expect("found");
        assert_eq!(summary.title.as_deref(), Some("port the parser (fork)"));
        assert_eq!(summary.cwd, cwd.to_string_lossy());
        assert_eq!(summary.messages, 3);
        assert!(!summary.archived);
        assert_eq!(store.list().expect("list").len(), 2);
    }

    #[test]
    fn a_fork_of_an_untitled_session_is_untitled() {
        let temp = TempDir::new().expect("temp");
        let store = open(&temp);
        let source = store.create(temp.path()).expect("create");

        let forked = store.fork(&source).expect("fork");

        assert_eq!(
            store.find(forked.as_str()).expect("find").unwrap().title,
            None
        );
        assert_eq!(store.list().expect("list").len(), 2);
    }

    #[test]
    fn a_whitespace_title_does_not_become_a_fork_title() {
        let temp = TempDir::new().expect("temp");
        let store = open(&temp);
        let source = store.create(temp.path()).expect("create");
        store.set_title(&source, Some("  ")).expect("title");

        let forked = store.fork(&source).expect("fork");

        assert_eq!(store.title(&forked).expect("title"), None);
    }

    #[test]
    fn a_fork_of_an_unknown_session_is_unknown() {
        let temp = TempDir::new().expect("temp");
        let store = open(&temp);

        assert!(matches!(
            store.fork(&super::SessionId("absent".to_owned())),
            Err(SessionError::UnknownSession { .. })
        ));
        assert!(
            store.list().expect("list").is_empty(),
            "nothing was written"
        );
    }

    #[test]
    fn archiving_sets_the_summary_flag_and_restoring_clears_it() {
        let temp = TempDir::new().expect("temp");
        let store = open(&temp);
        let session = store.create(temp.path()).expect("create");
        store.append(&session, "user", "keep me").expect("append");

        store.set_archived(&session, true).expect("archive");
        let summary = store.find(session.as_str()).expect("find").unwrap();
        assert!(summary.archived);
        assert_eq!(summary.messages, 1, "the transcript is untouched");
        assert_eq!(store.load(&session).expect("load").len(), 1);

        store.set_archived(&session, false).expect("restore");
        assert!(
            !store
                .find(session.as_str())
                .expect("find")
                .unwrap()
                .archived
        );
    }

    #[test]
    fn archiving_an_unknown_session_is_unknown() {
        let temp = TempDir::new().expect("temp");
        let store = open(&temp);

        assert!(matches!(
            store.set_archived(&super::SessionId("absent".to_owned()), true),
            Err(SessionError::UnknownSession { .. })
        ));
    }

    #[test]
    fn a_database_from_the_released_schema_gains_archived_at_without_losing_rows() {
        let temp = TempDir::new().expect("temp");
        let path: PathBuf = temp.path().join("sessions.db");
        // The schema the released 2.1.x binary wrote, before `archived_at`.
        {
            let connection = Connection::open(&path).expect("create the old store");
            connection
                .execute_batch(
                    "CREATE TABLE sessions (
                         id TEXT PRIMARY KEY,
                         created_at INTEGER NOT NULL,
                         title TEXT,
                         cwd TEXT NOT NULL
                     );
                     CREATE TABLE messages (
                         id INTEGER PRIMARY KEY AUTOINCREMENT,
                         session_id TEXT NOT NULL REFERENCES sessions(id),
                         role TEXT NOT NULL,
                         content TEXT NOT NULL,
                         created_at INTEGER NOT NULL
                     );
                     CREATE INDEX messages_session ON messages(session_id, id);
                     CREATE TABLE checkpoints (
                         session_id TEXT NOT NULL REFERENCES sessions(id),
                         label TEXT NOT NULL,
                         message_id INTEGER NOT NULL,
                         created_at INTEGER NOT NULL,
                         PRIMARY KEY (session_id, label)
                     );
                     INSERT INTO sessions (id, created_at, title, cwd)
                     VALUES ('2.1-session', 100, 'released', '/work');
                     INSERT INTO messages (session_id, role, content, created_at)
                     VALUES ('2.1-session', 'user', 'hello', 200),
                            ('2.1-session', 'assistant', 'hi', 300);",
                )
                .expect("seed the old store");
        }

        let store = SessionStore::open(&path).expect("migrate in place");
        let listed = store.list().expect("list");
        assert_eq!(listed.len(), 1, "no rows were lost");
        assert_eq!(listed[0].id.as_str(), "2.1-session");
        assert_eq!(listed[0].title.as_deref(), Some("released"));
        assert_eq!(listed[0].messages, 2);
        assert!(!listed[0].archived, "an old row starts live");
        assert_eq!(
            contents(&store, &listed[0].id),
            [
                ("user".to_owned(), "hello".to_owned(), 200),
                ("assistant".to_owned(), "hi".to_owned(), 300),
            ]
        );

        store.set_archived(&listed[0].id, true).expect("archive");
        drop(store);

        // Opening again runs the migration against a column that is there.
        let reopened = SessionStore::open(&path).expect("reopen");
        assert!(
            reopened
                .find("2.1-session")
                .expect("find")
                .unwrap()
                .archived
        );
        assert_eq!(contents(&reopened, &listed[0].id).len(), 2);
    }
}
