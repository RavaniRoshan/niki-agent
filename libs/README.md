# libs

Niki Agent is one package. Everything the product runs is in `code/`.

| Directory | What it is |
| --- | --- |
| [`code`](code/) | `niki-agent` — the terminal coding agent, its Textual TUI, headless mode, MCP, skills, the vendored SDK, and the ACP runtime. |
| [`partners/quickjs`](partners/quickjs/) | The JS interpreter middleware the agent depends on. Built from this repository. |
| [`../install`](../install/) | Release installers for Linux/macOS and Windows. |

Start with [`code/README.md`](code/README.md). For setup and the command
reference, see [`DEVELOPMENT.md`](DEVELOPMENT.md).
