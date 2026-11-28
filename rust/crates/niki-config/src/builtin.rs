//! Built-in provider blocks.
//!
//! A well-known provider resolves without a `[providers.*]` entry: the endpoint
//! and the key variable are part of the product. A user's `config.toml` only
//! names a default model, and this table supplies the rest without an extra
//! block.

use crate::{Protocol, Provider};

/// The endpoint and key variable for `name`, or `None` when the product ships no
/// default for it.
pub(crate) fn builtin(name: &str) -> Option<Provider> {
    let (base_url, api_key_env, protocol) = match name {
        "anthropic" => (
            "https://api.anthropic.com",
            "ANTHROPIC_API_KEY",
            Protocol::Anthropic,
        ),
        "openai" => (
            "https://api.openai.com/v1",
            "OPENAI_API_KEY",
            Protocol::OpenAi,
        ),
        "groq" => (
            "https://api.groq.com/openai/v1",
            "GROQ_API_KEY",
            Protocol::OpenAi,
        ),
        "openrouter" => (
            "https://openrouter.ai/api/v1",
            "OPENROUTER_API_KEY",
            Protocol::OpenAi,
        ),
        "together" => (
            "https://api.together.xyz/v1",
            "TOGETHER_API_KEY",
            Protocol::OpenAi,
        ),
        "deepseek" => (
            "https://api.deepseek.com/v1",
            "DEEPSEEK_API_KEY",
            Protocol::OpenAi,
        ),
        "nvidia" => (
            "https://integrate.api.nvidia.com/v1",
            "NVIDIA_API_KEY",
            Protocol::OpenAi,
        ),
        // The OpenAI-compatible surface of the Gemini API.
        "google_genai" => (
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "GOOGLE_API_KEY",
            Protocol::OpenAi,
        ),
        "ollama" => (
            "http://localhost:11434/v1",
            "OLLAMA_API_KEY",
            Protocol::OpenAi,
        ),
        "mistralai" => (
            "https://api.mistral.ai/v1",
            "MISTRAL_API_KEY",
            Protocol::OpenAi,
        ),
        "xai" => ("https://api.x.ai/v1", "XAI_API_KEY", Protocol::OpenAi),
        _ => return None,
    };

    Some(Provider {
        display_name: None,
        class_path: None,
        base_url: Some(base_url.to_owned()),
        api_key_env: Some(api_key_env.to_owned()),
        models: Vec::new(),
        protocol: Some(protocol_name(protocol).to_owned()),
        // A local daemon does not check the key it is sent.
        keyless: (name == "ollama").then_some(true),
        api_key: None,
    })
}

/// Every name with a built-in block, in the order the picker should show them.
pub(crate) const BUILTIN_NAMES: &[&str] = &[
    "anthropic",
    "openai",
    "google_genai",
    "ollama",
    "openrouter",
    "nvidia",
    "groq",
    "together",
    "deepseek",
    "mistralai",
    "xai",
];

fn protocol_name(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::OpenAi => "openai",
        Protocol::Anthropic => "anthropic",
    }
}
