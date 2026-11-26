use niki_provider::{StopReason, Usage};

/// Something that happened during a turn, reported as it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// A piece of answer text, to append to the transcript.
    Text(String),
    /// A tool call the model asked for, before it runs.
    ToolStart {
        /// Tool name as the model wrote it.
        name: String,
        /// Arguments as the model sent them, still a JSON string.
        arguments: String,
    },
    /// The result of a tool call, after it ran or failed.
    ToolEnd {
        /// Tool name.
        name: String,
        /// Text handed back to the model.
        output: String,
        /// Whether the output reports a failure rather than a result.
        is_error: bool,
    },
    /// Token accounting for one model turn.
    Usage(Usage),
    /// A message the person should see but that does not end the turn: a hook
    /// warning, or a hook's `systemMessage`.
    Notice(String),
    /// The step budget ran out before the model stopped asking for tools.
    StepLimit {
        /// Steps already used.
        steps: u32,
    },
    /// The cancel handle was raised; the transcript is kept.
    Cancelled,
}

/// What one [`Agent::run`](crate::Agent::run) call produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// The last answer text the model produced.
    pub text: String,
    /// Model turns taken.
    pub steps: u32,
    /// Tool calls executed.
    pub tool_calls: usize,
    /// Why the last model turn ended.
    pub stop: StopReason,
    /// Token usage for this run.
    pub usage: Usage,
}
