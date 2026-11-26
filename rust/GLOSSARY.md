# Glossary

The Python name on the left, the meaning, and its Rust name on the right. Add a
row when a unit introduces a term; never let two units name one thing
differently.

| Python | Meaning | Rust |
| --- | --- | --- |
| `RunEvent` | One envelope on the stream: text delta, tool call, tool result, usage, done. | `niki_proto::RunEvent` |
| `RunTurnCoordinator` | Drives one user turn end to end. | `niki_agent::Turn` |
| `AgentRuntime` | The application boundary around a streamed turn. | `niki_agent::Runtime` |
| `SessionSummary` | Title, timestamps, model, token totals. | `niki_session::Summary` |
| `TurnStatus` | Running, waiting-for-approval, done, cancelled, failed. | `niki_agent::TurnStatus` |
| `server_graph.py` | Composition root: provider + tools + policy in one loop. | `niki_agent::Agent` |
| approval mode | Manual, Auto, YOLO. | `niki_policy::Mode` |
| shell allow-list | Patterns a shell command must match to run unchallenged. | `niki_policy::AllowList` |
| workspace root | Detected project directory; tools cannot escape it. | `niki_workspace::Root` |
| tool call | Provider-requested invocation with JSON arguments. | `niki_proto::ToolCall` |
| tool result | The text returned to the model for one call. | `niki_proto::ToolResult` |
| MCP server | External tool provider over stdio or HTTP. | `niki_mcp::Server` |
| skill | A markdown instruction pack with front matter. | `niki_skills::Skill` |
| hook | A command run before or after a tool call. | `niki_hooks::Hook` |
| managed config | Administrator policy file that user config cannot override. | `niki_config::Managed` |
| transcript | The rendered conversation model. | `niki_render::Transcript` |
| frame | One deterministic plain-text render of a transcript. | `niki_render::Frame` |
