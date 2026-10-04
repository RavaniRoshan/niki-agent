# Niki Agent — Adding a provider

Nine of the twelve requested endpoints already ship as optional extras and need
nothing:

| Provider | Extra | Key env var |
| --- | --- | --- |
| Anthropic | `anthropic` | `ANTHROPIC_API_KEY` |
| OpenAI | `openai` | `OPENAI_API_KEY` |
| Google | `google-genai` | `GOOGLE_API_KEY` |
| Ollama | `ollama` | `OLLAMA_API_KEY` / `OLLAMA_HOST` |
| OpenRouter | `openrouter` | `OPENROUTER_API_KEY` |
| NVIDIA | `nvidia` | `NVIDIA_API_KEY` |
| Groq | `groq` | `GROQ_API_KEY` |
| Together | `together` | `TOGETHER_API_KEY` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` |

Install with `uv sync --all-extras`, or `uv sync --extra <name>` for one.

---

## The three that are not

Researched from each project's own source. **None of the three needs a new
runtime dependency** — all three are reachable through packages already in
`[project.optional-dependencies]`. They are not built-in `init_chat_model`
providers, so each is configured with `class_path` + `base_url`.

Drop the block you want into `~/.deepagents/config.toml`:

```toml
[providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://api.moonshot.ai/v1"     # or api.moonshot.cn/v1 in CN
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]

[providers.kilo]
display_name = "Kilo AI Gateway"
class_path = "langchain_openrouter:ChatOpenRouter"
base_url = "https://api.kilo.ai/api/gateway"
api_key_env = "KILO_API_KEY"
models = ["anthropic/claude-opus-4.7", "openai/gpt-5.4", "moonshotai/kimi-k2.5"]

[providers.opencode_zen]
display_name = "OpenCode Zen"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://opencode.ai/zen/v1"
api_key_env = "OPENCODE_API_KEY"
models = ["kimi-k3", "deepseek-v4-pro", "glm-5.3"]
```

### What each costs

**Kimi** — cleanest. One base URL per region, `MOONSHOT_API_KEY`, and Moonshot
documents both OpenAI *and* Anthropic wire compatibility, so either class works.
Two separate platforms exist and Moonshot's own docs warn against mixing them:
the *Kimi Code Plan* (`api.kimi.com/coding/v1`, `KIMI_API_KEY`) and the *Open
Platform* (`api.moonshot.ai/v1`, `MOONSHOT_API_KEY`). Documented quirk: a 429
from an exhausted balance is **not** retried — it fails immediately.

**Kilo** — cleanest wiring, one endpoint, one protocol, `langchain-openrouter`
already carries a `max_retries` entry. Documented quirks: free models are
limited to **200 req/hour per IP**; a zero balance returns **402**; request
bodies over 20 MB return **413**; and an upstream 402 is deliberately masked as
**503**. Worth knowing before you point a long-running task at it.

**OpenCode Zen** — the most awkward of the three. It serves **four different
wire protocols** depending on the model (`/chat/completions`, `/messages`,
`/responses`, and a Gemini-shaped route). One `base_url` therefore cannot cover
the catalog: the block above reaches the OpenAI-shaped subset only, and a second
entry with `langchain_anthropic:ChatAnthropic` would be needed for the Claude
models. Its model list also churns quickly, so a hardcoded `models = [...]`
becomes stale.

---

## Wiring them into `/auth`

The config.toml blocks above make the providers usable immediately via the model
selector. Putting them into `/auth`, the API-key display names, and the
retry-parameter map means editing five dicts in
`deepagents_code/model_config.py` (`PROVIDER_API_KEY_ENV`,
`PROVIDER_BASE_URL_ENV`, `PROVIDER_API_KEY_URLS`, `PROVIDER_DISPLAY_NAMES`,
`RETRY_PARAM_BY_PROVIDER`). That is an upstream patch, and it has not been made —
`config.toml` is the supported extension point and needs none. Say the word and
it is a small, logged change.

## What could not be verified

- Numeric RPM/TPM tiers for Kimi — Moonshot's docs SPA redirects every
  rate-limit page to its quickstart.
- Whether Kilo's `/api/gateway` serves `/chat/completions` unmodified for
  Anthropic-native models. No `/messages` route is documented, so the translation
  is presumably server-side, but that needs a key to confirm.
- Any of the three end-to-end against a live key. **No provider in this document
  has been exercised with a real credential.**