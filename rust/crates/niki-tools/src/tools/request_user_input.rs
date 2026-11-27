use serde_json::{Value, json};

use crate::ask::{AskAnswer, AskError, AskOption, AskRequest};
use crate::{Tool, ToolContext, ToolError, ToolOutput, args};

/// Longest question the model may ask, in characters.
pub const MAX_QUESTION_CHARS: usize = 2_000;

/// Fewest options a question must offer.
pub const MIN_OPTIONS: usize = 2;

/// Most options a question may offer.
pub const MAX_OPTIONS: usize = 4;

/// Ask the person a structured multiple-choice question.
///
/// With an [`Asker`](crate::Asker) installed on the [`ToolContext`] the call
/// blocks until the person answers and the result is their choice verbatim.
/// Without one the result explains that nobody is attached and lists the
/// options, so a headless run never waits.
pub struct RequestUserInput;

impl Tool for RequestUserInput {
    fn name(&self) -> &str {
        "request_user_input"
    }

    fn description(&self) -> &str {
        "Ask the person a multiple-choice question and return their answer. Use \
         it when a decision changes what you do next and prose would be missed."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_QUESTION_CHARS,
                    "description": "The question, in one or two sentences."
                },
                "options": {
                    "type": "array",
                    "minItems": MIN_OPTIONS,
                    "maxItems": MAX_OPTIONS,
                    "description": "Two to four choices.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": {
                                "type": "string",
                                "minLength": 1,
                                "description": "The answer text returned when this choice is picked."
                            },
                            "description": {
                                "type": "string",
                                "description": "One line explaining the choice."
                            }
                        },
                        "required": ["label"],
                        "additionalProperties": false
                    }
                },
                "allow_freeform": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether the person may type an answer instead of choosing."
                }
            },
            "required": ["question", "options"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let request = parse_request(&args)?;
        let Some(asker) = ctx.asker.as_ref() else {
            return Ok(ToolOutput::new(no_user_text(&request)));
        };
        match asker.ask(request.clone()) {
            Ok(AskAnswer::Choice(label)) => Ok(ToolOutput::new(label)),
            Ok(AskAnswer::Freeform(text)) => {
                if request.allow_freeform {
                    Ok(ToolOutput::new(text))
                } else {
                    Err(ToolError::Ask(
                        "the asker returned a free-form answer, but this question does not \
                         allow one; ask again and offer only the listed options"
                            .to_owned(),
                    ))
                }
            }
            Err(AskError::NoUser) => Ok(ToolOutput::new(no_user_text(&request))),
            Err(AskError::Failed(message)) => Err(ToolError::Ask(message)),
        }
    }
}

fn parse_request(args: &Value) -> Result<AskRequest, ToolError> {
    let question = args::required_str(args, "question")?;
    let length = question.chars().count();
    if question.trim().is_empty() {
        return Err(ToolError::InvalidArguments(
            "`question` must not be empty".to_owned(),
        ));
    }
    if length > MAX_QUESTION_CHARS {
        return Err(ToolError::InvalidArguments(format!(
            "`question` is {length} characters; the limit is {MAX_QUESTION_CHARS}"
        )));
    }

    Ok(AskRequest {
        question: question.to_owned(),
        options: parse_options(args)?,
        allow_freeform: args::optional_bool(args, "allow_freeform")?.unwrap_or(false),
    })
}

fn parse_options(args: &Value) -> Result<Vec<AskOption>, ToolError> {
    let Some(value) = args.get("options") else {
        return Err(ToolError::InvalidArguments(
            "missing required argument `options`".to_owned(),
        ));
    };
    let Some(items) = value.as_array() else {
        return Err(ToolError::InvalidArguments(
            "`options` must be an array".to_owned(),
        ));
    };
    if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&items.len()) {
        return Err(ToolError::InvalidArguments(format!(
            "`options` must list {MIN_OPTIONS} to {MAX_OPTIONS} choices, got {}",
            items.len()
        )));
    }

    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let Some(object) = item.as_object() else {
                return Err(ToolError::InvalidArguments(format!(
                    "`options[{index}]` must be an object"
                )));
            };
            let label = match object.get("label") {
                Some(Value::String(label)) if !label.trim().is_empty() => label.clone(),
                Some(Value::String(_)) => {
                    return Err(ToolError::InvalidArguments(format!(
                        "`options[{index}].label` must not be empty"
                    )));
                }
                Some(_) => {
                    return Err(ToolError::InvalidArguments(format!(
                        "`options[{index}].label` must be a string"
                    )));
                }
                None => {
                    return Err(ToolError::InvalidArguments(format!(
                        "`options[{index}]` is missing `label`"
                    )));
                }
            };
            let description = match object.get("description") {
                None => None,
                Some(Value::String(description)) => Some(description.clone()),
                Some(_) => {
                    return Err(ToolError::InvalidArguments(format!(
                        "`options[{index}].description` must be a string"
                    )));
                }
            };
            Ok(AskOption { label, description })
        })
        .collect()
}

fn no_user_text(request: &AskRequest) -> String {
    let mut text = String::from(
        "No interactive user is attached to this run, so the question was not shown. \
         Ask in prose instead. The options were:\n",
    );
    for option in &request.options {
        text.push_str("- ");
        text.push_str(&option.label);
        if let Some(description) = &option.description {
            text.push_str(" — ");
            text.push_str(description);
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::ScriptedAsker;

    fn workspace() -> (TempDir, ToolContext) {
        let dir = tempfile::tempdir().expect("tempdir");
        let ctx = ToolContext::new(dir.path()).expect("workspace");
        (dir, ctx)
    }

    async fn ask(ctx: &ToolContext, args: Value) -> Result<ToolOutput, ToolError> {
        Tool::run(&RequestUserInput, args, ctx).await
    }

    fn two_options() -> Value {
        json!([
            {"label": "SQLite", "description": "one file, no server"},
            {"label": "JSON", "description": "a file per record"}
        ])
    }

    #[tokio::test]
    async fn rejects_fewer_than_two_options() {
        let (_dir, ctx) = workspace();
        for options in [json!([]), json!([{"label": "only"}])] {
            let error = ask(
                &ctx,
                json!({"question": "Which store?", "options": options}),
            )
            .await
            .expect_err("fewer than two options is refused");
            assert!(error.to_string().contains("2 to 4"), "{error}");
        }
    }

    #[tokio::test]
    async fn rejects_more_than_four_options() {
        let (_dir, ctx) = workspace();
        let options = json!([
            {"label": "a"},
            {"label": "b"},
            {"label": "c"},
            {"label": "d"},
            {"label": "e"}
        ]);

        let error = ask(&ctx, json!({"question": "Pick", "options": options}))
            .await
            .expect_err("five options is refused");

        assert!(error.to_string().contains("2 to 4"), "{error}");
    }

    #[tokio::test]
    async fn rejects_a_question_over_the_length_limit() {
        let (_dir, ctx) = workspace();
        let question = "q".repeat(MAX_QUESTION_CHARS + 1);

        let error = ask(
            &ctx,
            json!({"question": question, "options": two_options()}),
        )
        .await
        .expect_err("an overlong question is refused");

        assert!(error.to_string().contains("2000"), "{error}");
    }

    #[tokio::test]
    async fn accepts_a_question_at_the_length_limit() {
        let (_dir, ctx) = workspace();
        let question = "q".repeat(MAX_QUESTION_CHARS);

        let output = ask(
            &ctx,
            json!({"question": question, "options": two_options()}),
        )
        .await
        .expect("a question at the limit is accepted");

        assert!(output.text.contains("No interactive user"));
    }

    #[tokio::test]
    async fn without_an_asker_lists_the_options_instead_of_waiting() {
        let (_dir, ctx) = workspace();

        let output = ask(
            &ctx,
            json!({"question": "Which store?", "options": two_options()}),
        )
        .await
        .expect("no asker is a normal result");

        assert!(output.text.contains("No interactive user is attached"));
        assert!(output.text.contains("Ask in prose"));
        assert!(output.text.contains("SQLite"), "{}", output.text);
        assert!(output.text.contains("JSON"), "{}", output.text);
    }

    #[tokio::test]
    async fn a_chosen_option_is_returned_verbatim() {
        let (_dir, ctx) = workspace();
        let asker = Arc::new(ScriptedAsker::new([AskAnswer::Choice("SQLite".to_owned())]));
        let ctx = ctx.with_asker(asker.clone());

        let output = ask(
            &ctx,
            json!({"question": "Which store?", "options": two_options()}),
        )
        .await
        .expect("the asker answers");

        assert_eq!(output.text, "SQLite");
        let requests = asker.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].question, "Which store?");
        assert!(!requests[0].allow_freeform);
        assert_eq!(requests[0].options[0].label, "SQLite");
        assert_eq!(
            requests[0].options[0].description.as_deref(),
            Some("one file, no server")
        );
    }

    #[tokio::test]
    async fn freeform_is_rejected_when_not_allowed() {
        let (_dir, ctx) = workspace();
        let ctx = ctx.with_asker(Arc::new(ScriptedAsker::new([AskAnswer::Freeform(
            "something else".to_owned(),
        )])));

        let error = ask(
            &ctx,
            json!({"question": "Which store?", "options": two_options()}),
        )
        .await
        .expect_err("free text is not offered");

        assert!(error.to_string().contains("free-form"), "{error}");
    }

    #[tokio::test]
    async fn freeform_is_accepted_when_allowed() {
        let (_dir, ctx) = workspace();
        let ctx = ctx.with_asker(Arc::new(ScriptedAsker::new([AskAnswer::Freeform(
            "SQLite, with WAL".to_owned(),
        )])));

        let output = ask(
            &ctx,
            json!({
                "question": "Which store?",
                "options": two_options(),
                "allow_freeform": true
            }),
        )
        .await
        .expect("free text is offered");

        assert_eq!(output.text, "SQLite, with WAL");
    }

    #[tokio::test]
    async fn an_asker_without_a_user_falls_back_to_the_same_explanation() {
        let (_dir, ctx) = workspace();
        let ctx = ctx.with_asker(Arc::new(ScriptedAsker::failing(AskError::NoUser)));

        let output = ask(
            &ctx,
            json!({"question": "Which store?", "options": two_options()}),
        )
        .await
        .expect("nobody attached is a normal result");

        assert!(output.text.contains("No interactive user is attached"));
        assert!(output.text.contains("SQLite"));
    }

    #[tokio::test]
    async fn an_asker_failure_is_a_tool_error() {
        let (_dir, ctx) = workspace();
        let ctx = ctx.with_asker(Arc::new(ScriptedAsker::failing(AskError::Failed(
            "the person dismissed the question".to_owned(),
        ))));

        let error = ask(
            &ctx,
            json!({"question": "Which store?", "options": two_options()}),
        )
        .await
        .expect_err("a failed ask is reported");

        assert!(
            error
                .to_string()
                .contains("the person dismissed the question"),
            "{error}"
        );
    }
}
