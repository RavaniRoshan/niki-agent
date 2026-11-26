# Providers

Niki Agent speaks two wire protocols: OpenAI Chat Completions and Anthropic
Messages. Eleven providers have a built-in endpoint and key variable, so a
configuration that only names a default model still runs. Every provider is a
`[providers.<name>]` table in `config.toml`; the built-ins are defaults a user
block overrides field by field.

The configuration file is `$NIKI_HOME/config.toml` when `NIKI_HOME` is set,
otherwise `~/.niki/config.toml`. `--config <PATH>` selects another file.

## Built-in providers

| Table name | Protocol | Endpoint | Key environment variable |
| --- | --- | --- | --- |
| `anthropic` | Anthropic Messages | `https://api.anthropic.com` | `ANTHROPIC_API_KEY` |
| `openai` | OpenAI Chat Completions | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| `google_genai` | OpenAI Chat Completions | `https://generativelanguage.googleapis.com/v1beta/openai` | `GOOGLE_API_KEY` |
| `ollama` | OpenAI Chat Completions | `http://localhost:11434/v1` | none; keyless |
| `openrouter` | OpenAI Chat Completions | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
| `nvidia` | OpenAI Chat Completions | `https://integrate.api.nvidia.com/v1` | `NVIDIA_API_KEY` |
| `groq` | OpenAI Chat Completions | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` |
| `together` | OpenAI Chat Completions | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` |
| `deepseek` | OpenAI Chat Completions | `https://api.deepseek.com/v1` | `DEEPSEEK_API_KEY` |
| `mistralai` | OpenAI Chat Completions | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` |
| `xai` | OpenAI Chat Completions | `https://api.x.ai/v1` | `XAI_API_KEY` |

The key is read from the environment named by `api_key_env`. Niki reads it when
it builds the provider for a turn; it never writes a key to disk.

`google_genai` uses the OpenAI-compatible surface of the Gemini API, so it takes
`GOOGLE_API_KEY` and the OpenAI request shape. `ollama` is keyless: it declares
`OLLAMA_API_KEY`, but the local daemon ignores the credential and the value is
optional. Niki talks to `http://localhost:11434/v1` unless `base_url` points
somewhere else.

## Select a model

A model spec is `provider:model`. Set the default in the configuration, or pass
one for a single run:

```toml
# ~/.niki/config.toml
[models]
default = "anthropic:my-model"
```

```bash
niki --model groq:llama-3.3-70b
```

Inside the full-screen app, `/model` opens a picker and `/model provider:model`
switches directly. The picker offers `[models].default` first, then every
`provider:model` pair listed in a user provider block's `models` array. A
built-in provider has no model list of its own, so it runs from `--model` or
`[models].default` even though the picker does not browse it.

An unknown provider or a spec without a colon is an error. The unknown provider
message lists every name the configuration can resolve.

## Provider block

A `[providers.<name>]` table (also accepted at `[models.providers.<name>]`)
accepts these keys. Every one is optional; a user block overrides the fields it
sets and inherits the rest of a built-in block.

| Key | Read as |
| --- | --- |
| `display_name` | The label the model picker shows. Defaults to the table name. |
| `models` | Model identifiers this provider offers, for the picker. |
| `base_url` | Endpoint base URL. Defaults to the protocol's endpoint. |
| `api_key_env` | Name of the environment variable that holds the key. |
| `protocol` | `openai`, `openai-compatible`, `openai_compatible`, or `anthropic`. |
| `class_path` | Read only to infer the protocol when `protocol` is absent: a value containing `anthropic` selects Anthropic Messages, anything else selects OpenAI Chat Completions. It is not imported or executed. |
| `keyless` | `true` when the endpoint needs no credential. Built-in `ollama` sets it. |

Unknown keys are ignored, so a file shared with another Niki surface loads
without error. An explicit `protocol` this build does not recognise falls back
to the `class_path` inference and then to OpenAI Chat Completions.

## Any OpenAI-compatible endpoint

A gateway that speaks OpenAI Chat Completions is a configuration block, not a
code change:

```toml
[providers.kimi]
display_name = "Kimi"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
protocol = "openai"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

A gateway that serves the Anthropic Messages wire uses the other protocol:

```toml
[providers.gateway]
display_name = "Anthropic gateway"
base_url = "https://gateway.example.com/anthropic"
api_key_env = "GATEWAY_API_KEY"
protocol = "anthropic"
models = ["anthropic-large", "anthropic-small"]
```

A gateway that serves more than one wire protocol needs one block per protocol,
because `protocol` picks the adapter. Keep the model IDs exactly as the gateway
serves them; the picker only offers the IDs listed in `models`.

## Local models

Ollama is the shortest route:

```bash
ollama pull qwen3-coder
export OLLAMA_HOST=http://localhost:11434   # only if the daemon is not local
```

```toml
[models]
default = "ollama:qwen3-coder"
```

Any other local server that exposes an OpenAI-compatible API is a provider block
with its host as `base_url`; set `keyless = true` if it accepts no credential.

## Troubleshooting

- **`unknown provider`** — the name in `--model` or `[models].default` is not a
  built-in and has no `[providers.<name>]` block. The error lists every name
  that resolves.
- **`missing key`** — the variable named by that provider's `api_key_env` is
  unset or empty. The error names both the variable and the provider.
- **`401` or `403` from a gateway** — the key is wrong, or the endpoint expects
  the other wire protocol. Change `protocol` and try again.
- **A model ID is rejected** — add the exact ID your gateway serves to `models`,
  and select it as `provider:model`.
- **A built-in provider has no models in the picker** — the picker lists only
  `[models].default` and user block `models`. Pass `--model provider:model` or
  add a block with a `models` list.
