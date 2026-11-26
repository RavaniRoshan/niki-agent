<!--
  Mirrored from docs/niki/PROVIDERS.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: d6d4cb6b7cfd7a74d798366f6715fc755cda8edf30d2c22de144b50bdd9daa06
-->
# Providers

Niki Agent talks to model providers through a small client integration per
vendor. Anthropic, OpenAI, and Google Gemini are installed with the app; every
other integration is an optional extra you enable when you need it.

## Install an integration

In an installed release, run `/install <extra>` inside a session:

```text
/install ollama
/install openrouter
```

From a source checkout, extras are installed with uv instead:

```bash
cd libs/code
uv sync --extra <name>          # one integration
uv sync --all-extras            # providers and sandboxes
```

| Provider | Extra | Key environment variable |
| --- | --- | --- |
| Anthropic | `anthropic` | `ANTHROPIC_API_KEY` |
| OpenAI | `openai` | `OPENAI_API_KEY` |
| Azure OpenAI | `openai` | `AZURE_OPENAI_API_KEY` |
| Google Gemini | `google-genai` | `GOOGLE_API_KEY` |
| Google Vertex AI | `vertex` | `GOOGLE_CLOUD_PROJECT` (Application Default Credentials) |
| Amazon Bedrock | `bedrock` | AWS credentials |
| Ollama | `ollama` | none; `OLLAMA_HOST` is optional |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` |
| NVIDIA | `nvidia` | `NVIDIA_API_KEY` |
| Groq | `groq` | `GROQ_API_KEY` |
| Together AI | `together` | `TOGETHER_API_KEY` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` |
| Mistral AI | `mistralai` | `MISTRAL_API_KEY` |
| Cohere | `cohere` | `COHERE_API_KEY` |
| xAI | `xai` | `XAI_API_KEY` |
| Perplexity | `perplexity` | `PPLX_API_KEY` |
| Fireworks | `fireworks` | `FIREWORKS_API_KEY` |
| Hugging Face | `huggingface` | `HUGGINGFACEHUB_API_TOKEN` |
| IBM watsonx | `ibm` | `WATSONX_APIKEY` |
| Meta | `meta` | `MODEL_API_KEY` |
| Baseten | `baseten` | `BASETEN_API_KEY` |
| LiteLLM | `litellm` | `LITELLM_API_KEY` |

`/auth` in a session is the interactive path: it shows which providers are
configured, stores a key when you paste one, and links to the provider's key
page. A key you paste there is written to `~/.niki/.state/auth.json` (mode
`0600`); an environment variable is never rewritten.

Ollama needs no key. Niki looks for a local server at `http://localhost:11434`
unless `OLLAMA_HOST` or `base_url` points somewhere else, and it discovers the
models you have pulled.

## Any OpenAI-compatible endpoint

A gateway that speaks the OpenAI chat-completions API is a configuration block,
not a code change. Add it to `~/.niki/config.toml` and it appears in `/model`:

```toml
[models.providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

`class_path` is a Python class in `module.path:ClassName` form. Niki imports it
and instantiates it, so it must be a chat-model class (a `BaseChatModel`
subclass). The value is the literal string the field accepts — it names the
installed client package that speaks the gateway's protocol, not a preference.
`api_key_env` names the environment variable to read, never the key itself;
`/auth` and the startup check use the same name.

A gateway that serves more than one wire protocol needs one provider block per
protocol, because `class_path` picks the protocol. Two more examples:

```toml
[models.providers.kilo]
display_name = "Kilo AI Gateway"
class_path = "langchain_openrouter:ChatOpenRouter"
base_url = "https://api.kilo.ai/api/gateway"
api_key_env = "KILO_API_KEY"
models = ["anthropic/claude-opus-4.7", "openai/gpt-5.4", "moonshotai/kimi-k2.5"]

[models.providers.opencode_zen]
display_name = "OpenCode Zen"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://opencode.ai/zen/v1"
api_key_env = "OPENCODE_API_KEY"
models = ["kimi-k3", "deepseek-v4-pro", "glm-5.3"]
```

The `opencode_zen` block reaches the OpenAI-shaped part of that catalog. Models
served over the Anthropic messages wire would need a second entry using
`langchain_anthropic:ChatAnthropic`.

## The provider table

Every key in `[models.providers.<name>]` is optional except the ones your
gateway needs.

| Key | Meaning |
| --- | --- |
| `display_name` | The label shown in `/model` and `/auth`. |
| `short_name` | A shorter label for tight spaces; falls back to `display_name`. |
| `enabled` | Set `false` to hide an installed provider and its models. |
| `models` | The model IDs this provider offers. |
| `class_path` | The chat-model class to instantiate, instead of the built-in provider path. |
| `base_url` | The API endpoint. |
| `base_url_env` | An environment variable that supplies the endpoint. |
| `api_key_env` | The environment variable that holds the key. |
| `api_key_url` | Where a user creates a key, linked from `/auth`. |
| `params` | Extra constructor arguments, applied to every model. A model-keyed sub-table overrides one model. |

`class_path` imports code from your own configuration, so treat a config file
from somewhere else the way you would treat a `pyproject.toml` build script.

## Local models

Ollama is the shortest route:

```bash
ollama pull qwen3-coder
uv sync --extra ollama
```

Then pick the model in `/model`, or set the default in `~/.niki/config.toml`:

```toml
[models]
default = "ollama:qwen3-coder"
```

For any other local server that exposes an OpenAI-compatible API, use the
`class_path` block above with its host as `base_url`.

## Troubleshooting

- **A provider does not appear in `/model`** — its integration is not installed
  (`/install <extra>`, or `uv sync --extra <name>` in a checkout), or it is
  disabled with `enabled = false`.
- **`401` or `403` from a gateway** — the environment variable named by
  `api_key_env` is not set and `/auth` has no stored key for that provider, or
  the endpoint expects a different wire protocol.
- **A model ID is rejected** — add the exact ID your gateway serves to `models`.
  The selector only offers IDs listed there.

---

Source: [`docs/niki/PROVIDERS.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/PROVIDERS.md) in the Niki Agent repository.
