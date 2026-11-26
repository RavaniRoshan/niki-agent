use niki_provider::ProviderError;

/// Why a turn could not finish.
///
/// A tool failure is not an error here: it becomes a tool result the model can
/// react to, which is what the Python product does. Only failures that end the
/// run surface as an `AgentError`.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The model request failed, or its stream broke mid-turn.
    #[error("provider {provider} failed: {source}")]
    Provider {
        /// Provider name, so the message says which endpoint failed (RULES.md 5).
        provider: String,
        /// The transport or decode failure.
        #[source]
        source: ProviderError,
    },
}
