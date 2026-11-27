# Providers

Niki Agent speaks two wire protocols: the OpenAI chat-completions API and the
Anthropic messages API. Eleven providers have a built-in block — an endpoint and
a key variable that ship with the binary — so naming one of them in
`config.toml` is all an install needs. Any other endpoint that speaks one of the
two protocols is a `[providers.<name>]` block.

There is nothing to install and no Python package behind a provider: the client
is compiled in.

The configuration file is `$NIKI_HOME/config.toml` when `NIKI_HOME` is set,
otherwise `~/.niki/config.toml`. `--config <PATH>` selects another file.

## Built-in providers

| Name | Key environment variable | Endpoint |
| --- | --- | --- |
| `anthropic` | `ANTHROPIC_API_KEY` | `https://api.anthropic.com` (Anthropic messages) |
| `openai` | `OPENAI_API_KEY` | `https://api.openai.com/v1` |
| `google_genai` | `GOOGLE_API_KEY` | `https://generativelanguage.googleapis.com/v1beta/openai` |
| `ollama` | none | `http://localhost:11434/v1` |
| `openrouter` | `OPENROUTER_API_KEY` | `https://openrouter.ai/api/v1` |
| `nvidia` | `NVIDIA_API_KEY` | `https://integrate.api.nvidia.com/v1` |
| `groq` | `GROQ_API_KEY` | `https://api.groq.com/openai/v1` |
| `together` | `TOGETHER_API_KEY` | `https://api.together.xyz/v1` |
| `deepseek` | `DEEPSEEK_API_KEY` | `https://api.deepseek.com/v1` |
| `mistralai` | `MISTRAL_API_KEY` | `https://api.mistral.ai/v1` |
| `xai` | `XAI_API_KEY` | `https://api.x.ai/v1` |

Every provider except Anthropic uses the OpenAI-compatible protocol; Anthropic
uses its own messages API. Google Gemini is reached through the OpenAI-compatible
surface of the Gemini API, so it needs no separate client.

An environment variable is the only place a key is read from. Export it in the
shell that starts `niki`:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki --model anthropic:my-model -n "summarize this repo"
```

Ollama needs no key: it is a local daemon, and the client treats it as keyless.
Set `base_url` in a `[providers.ollama]` block to point at a server on another
machine.

## Choosing a model

`--model <provider:model>` selects one for a run; `/model` picks in the app.
Without either, `[models].default` decides:

```toml
[models]
default = "groq:llama-3.3-70b-versatile"
```

The picker reads `[models].default` first, then every `provider:model` pair in a
provider block's `models` array:

```toml
[providers.groq]
models = ["llama-3.3-70b-versatile", "moonshotai/kimi-k2-instruct"]
```

A built-in provider has no model list of its own, so it runs from `--model` or
the default but does not appear in the picker unless a block adds `models`.

## Any OpenAI-compatible endpoint

A gateway that speaks the chat-completions API is a configuration block, not a
code change. Add it to `~/.niki/config.toml` and it appears in `/model`:

```toml
[providers.kimi]
display_name = "Kimi"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

A gateway that serves the Anthropic messages wire instead sets the protocol:

```toml
[providers.gateway]
display_name = "Shared Gateway"
protocol = "anthropic"
base_url = "https://gateway.example/v1"
api_key_env = "GATEWAY_API_KEY"
models = ["anthropic-large", "anthropic-small"]
```

`protocol` takes `openai` or `anthropic`; `openai-compatible` and
`openai_compatible` are also accepted. Without it, the protocol is inferred from
the block and defaults to OpenAI-compatible, which is right for most gateways.

## The provider table

Every key in `[providers.<name>]` is optional except the ones a gateway needs.

| Key | Meaning |
| --- | --- |
| `display_name` | The label shown in `/model`. |
| `models` | The model IDs this provider offers. |
| `base_url` | The API endpoint. |
| `api_key_env` | The environment variable that holds the key. Names the variable, never the key. |
| `protocol` | `openai` or `anthropic`. Overrides the inferred protocol. |
| `keyless` | `true` for an endpoint that checks no credential, such as a local daemon. |

Unknown keys are ignored, so a file written for a later release still loads.

## When a key is missing

A key is never written to disk by the client; it is read from the environment
each run. Naming a provider whose key variable is unset fails before the request
leaves the machine, with the variable to set:

```text
niki: groq needs an API key: set GROQ_API_KEY
```

The exit status is 1. The picker does not check keys, so a provider with
`models` still appears when its key is unset — starting a turn with it reports
the same error.
