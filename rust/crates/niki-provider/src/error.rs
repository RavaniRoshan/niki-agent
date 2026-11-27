//! Provider failures, each naming what the user can act on.

use thiserror::Error;

/// A request to a model provider failed.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// The provider answered with a non-success status that retrying will not
    /// fix.
    #[error("{provider} returned HTTP {status}: {body}")]
    Http {
        /// Provider name, so a multi-provider session says which one failed.
        provider: String,
        /// Status code.
        status: u16,
        /// Response body, trimmed by the caller to a useful prefix.
        body: String,
    },
    /// The connection failed, or the response was cut short.
    #[error("{provider} could not be reached: {detail}")]
    Transport {
        /// Provider name.
        provider: String,
        /// Underlying message.
        detail: String,
    },
    /// The stream carried bytes this adapter does not understand.
    #[error("{provider} sent a stream this client cannot read: {detail}")]
    Decode {
        /// Provider name.
        provider: String,
        /// What was wrong.
        detail: String,
    },
    /// The provider block names a key variable that is unset or empty.
    #[error("{provider} needs an API key: set {var}")]
    MissingKey {
        /// Provider name.
        provider: String,
        /// Environment variable the provider block names.
        var: String,
    },
}

impl ProviderError {
    /// Whether the failure is worth another attempt.
    ///
    /// Retrying only helps when nothing was accepted: a 408, 409, 425, 429 or a
    /// transient 5xx before the stream started. Everything else is the user's
    /// configuration or the provider's permanent answer. See
    /// [`crate::retry::is_retryable`] for the policy the clients use.
    #[must_use]
    pub fn is_retryable_status(status: u16) -> bool {
        crate::retry::is_retryable_status(status)
    }
}
