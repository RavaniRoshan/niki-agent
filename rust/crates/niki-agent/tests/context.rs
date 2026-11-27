//! Context accounting and compaction, driven by a scripted provider.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_agent::{Agent, AgentConfig, AgentError, Compaction, Message, Role, estimate_tokens};
use niki_provider::{ChatRequest, Provider, ProviderError, StopReason, StreamEvent, Usage};
use niki_tools::ToolContext;

/// One scripted turn: its events, or the failure that stands where a reply
/// would.
type Turn = Vec<Result<StreamEvent, ProviderError>>;

/// A provider that replays one scripted turn per request.
#[derive(Clone)]
struct Scripted {
    turns: Arc<Mutex<VecDeque<Turn>>>,
    seen: Arc<Mutex<Vec<ChatRequest>>>,
}

impl Scripted {
    fn new(turns: Vec<Turn>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns.into())),
            seen: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<ChatRequest> {
        self.seen.lock().expect("lock").clone()
    }
}

impl Provider for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        self.seen.lock().expect("lock").push(request);
        let events = self
            .turns
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or_default();
        futures_util::stream::iter(events).boxed()
    }
}

fn reply(text: &str, input_tokens: u64, output_tokens: u64) -> Turn {
    vec![
        Ok(StreamEvent::TextDelta(text.to_owned())),
        Ok(StreamEvent::Usage(Usage {
            input_tokens,
            output_tokens,
        })),
        Ok(StreamEvent::Done(StopReason::EndTurn)),
    ]
}

fn failure() -> Turn {
    vec![Err(ProviderError::Transport {
        provider: "scripted".to_owned(),
        detail: "connection reset".to_owned(),
    })]
}

fn config() -> AgentConfig {
    AgentConfig {
        model: "test-model".to_owned(),
        ..AgentConfig::default()
    }
}

fn agent(provider: Scripted, root: &std::path::Path) -> Agent {
    Agent::new(
        Box::new(provider),
        ToolContext::new(root).expect("workspace"),
        config(),
    )
}

#[tokio::test]
async fn usage_and_steps_accumulate_across_turns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![reply("one", 11, 3), reply("two", 7, 5)]);
    let mut agent = agent(provider, dir.path());

    agent.run("first", &mut |_| {}).await.expect("run");
    let report = agent.context_report();
    assert_eq!(report.steps, 1);
    assert_eq!(report.last_input_tokens, 11);
    assert_eq!(report.last_output_tokens, 3);

    agent.run("second", &mut |_| {}).await.expect("run");

    let usage = agent.session_usage();
    assert_eq!(usage.input_tokens, 18);
    assert_eq!(usage.output_tokens, 8);
    assert_eq!(usage.steps, 2);
    assert_eq!(usage.total_tokens(), 26);
    assert_eq!(
        agent.usage(),
        Usage {
            input_tokens: 18,
            output_tokens: 8,
        }
    );

    let report = agent.context_report();
    assert_eq!(report.messages, 4);
    assert_eq!(report.steps, 2);
    assert_eq!(report.last_input_tokens, 7);
    assert_eq!(report.last_output_tokens, 5);
    assert_eq!(report.estimated_tokens, estimate_tokens(agent.messages()));
}

#[tokio::test]
async fn compaction_replaces_the_older_turns_exactly_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        reply("a1", 1, 1),
        reply("a2", 1, 1),
        reply("a3", 1, 1),
        reply("SUMMARY of the first two turns", 20, 9),
    ]);
    let mut agent = agent(provider.clone(), dir.path());
    for prompt in ["first", "second", "third"] {
        agent.run(prompt, &mut |_| {}).await.expect("run");
    }
    assert_eq!(agent.messages().len(), 6);

    let compaction = agent.compact(&provider, 1).await.expect("compact");

    let messages = agent.messages();
    assert_eq!(compaction.replaced, 4);
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].role, Role::Assistant);
    assert!(
        messages[0]
            .content
            .contains("SUMMARY of the first two turns")
    );
    assert_eq!(messages[1].content, "third");
    assert_eq!(messages[2].content, "a3");
    assert_eq!(compaction.summary_tokens, estimate_tokens(&messages[..1]));

    let requests = provider.requests();
    assert_eq!(requests.len(), 4);
    let request = requests.last().expect("summary request");
    assert_eq!(request.messages.len(), 5);
    assert_eq!(
        request.messages[4].content,
        "Summarize the conversation so far for another assistant to continue. \
         Keep decisions, file paths and open tasks. Do not add commentary."
    );
    assert!(request.tools.is_empty());

    assert_eq!(agent.session_usage().input_tokens, 23);
    assert_eq!(agent.session_usage().output_tokens, 12);
    assert_eq!(agent.session_usage().steps, 3);

    let again = agent.compact(&provider, 2).await.expect("compact");
    assert_eq!(again, Compaction::default());
    assert_eq!(agent.messages().len(), 3);
    assert_eq!(provider.requests().len(), 4, "nothing older to send");
}

#[tokio::test]
async fn a_provider_failure_leaves_the_history_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        reply("a1", 1, 1),
        reply("a2", 1, 1),
        reply("a3", 1, 1),
        failure(),
    ]);
    let mut agent = agent(provider.clone(), dir.path());
    for prompt in ["first", "second", "third"] {
        agent.run(prompt, &mut |_| {}).await.expect("run");
    }
    let before = agent.messages().to_vec();

    let error = agent
        .compact(&provider, 1)
        .await
        .expect_err("provider fails");

    assert!(matches!(error, AgentError::Provider { .. }));
    assert_eq!(agent.messages(), before.as_slice());
}

#[tokio::test]
async fn compaction_without_a_model_keeps_the_last_turns_verbatim() {
    let history = vec![
        Message::user("one"),
        Message::assistant("first"),
        Message::user("two"),
        Message::assistant("second"),
        Message::user("three"),
        Message::assistant("third"),
        Message::user("four"),
        Message::assistant("fourth"),
    ];
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(Vec::new());
    let mut agent = agent(provider, dir.path()).with_history(history.clone());

    assert_eq!(agent.compact_without_model(9), Compaction::default());
    assert_eq!(agent.messages(), history.as_slice());

    let compaction = agent.compact_without_model(2);

    assert_eq!(compaction.replaced, 4);
    let messages = agent.messages();
    assert_eq!(messages.len(), 5);
    assert_eq!(messages[0].role, Role::Assistant);
    assert!(
        messages[0]
            .content
            .starts_with("4 earlier messages were dropped"),
        "{}",
        messages[0].content
    );
    assert_eq!(&messages[1..], &history[4..]);
    assert_eq!(compaction.summary_tokens, estimate_tokens(&messages[..1]));
}

#[tokio::test]
async fn compaction_without_a_model_names_a_single_dropped_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(Vec::new());
    let mut agent = agent(provider, dir.path()).with_history(vec![
        Message::assistant("old summary"),
        Message::user("go on"),
        Message::assistant("done"),
    ]);

    let compaction = agent.compact_without_model(1);

    assert_eq!(compaction.replaced, 1);
    assert_eq!(agent.messages().len(), 3);
    assert!(
        agent.messages()[0]
            .content
            .starts_with("1 earlier message was dropped"),
        "{}",
        agent.messages()[0].content
    );
    assert_eq!(agent.messages()[1].content, "go on");
    assert_eq!(agent.messages()[2].content, "done");
}

#[tokio::test]
async fn compaction_keeps_the_last_n_turns_of_a_saved_history() {
    let history = vec![
        Message::user("one"),
        Message::assistant("first"),
        Message::user("two"),
        Message::assistant("second"),
        Message::user("three"),
        Message::assistant("third"),
    ];
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![reply("kept", 1, 1)]);
    let mut agent = agent(provider.clone(), dir.path()).with_history(history.clone());

    let compaction = agent.compact(&provider, 2).await.expect("compact");

    assert_eq!(compaction.replaced, 2);
    let messages = agent.messages();
    assert_eq!(messages.len(), 5);
    assert!(messages[0].content.contains("kept"));
    assert_eq!(&messages[1..], &history[2..]);
}
