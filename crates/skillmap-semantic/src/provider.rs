//! The one thing that talks to a model.
//!
//! `docs/04-semantic-layer.md`: *"No tools, no network, no filesystem available
//! to the pass beyond the single model call. Nothing the model emits can cause
//! an action."*
//!
//! That is enforced by the shape of [`Provider`] rather than by care. The trait
//! takes a string and returns a string. There is no tool list to populate, no
//! callback for the model to reach back through, and no second round trip — a
//! provider that wanted to give the model tools would have to change this
//! signature, in a diff somebody reads.

/// Why a model call did not produce an answer.
#[derive(Debug)]
pub enum ProviderError {
    /// The call did not complete: transport, timeout, auth, rate limit.
    Call(String),
    /// The provider is compiled out of this build.
    NotCompiledIn(&'static str),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Call(why) => write!(f, "{why}"),
            Self::NotCompiledIn(feature) => write!(
                f,
                "this build has no model provider; rebuild with --features {feature}"
            ),
        }
    }
}

impl std::error::Error for ProviderError {}

/// A model, reduced to its smallest useful surface.
pub trait Provider {
    /// Model id, pinned into `advisory.model`.
    fn model(&self) -> &str;

    /// One completion. One call. No tools.
    ///
    /// # Errors
    ///
    /// [`ProviderError`] if the call did not complete. A failed call becomes a
    /// `semantic_call_failed` diagnostic and the advisory branch reports
    /// **disabled** — never "ran, found nothing", which is a different claim.
    fn complete(&self, prompt: &str) -> Result<String, ProviderError>;
}

/// A provider that replays a fixed response.
///
/// For tests and for the structural quarantine proof, where the point is to
/// control exactly what the model says — including saying something hostile —
/// and observe that the deterministic branches do not move.
#[derive(Debug, Clone)]
pub struct Replay {
    model: String,
    response: String,
}

impl Replay {
    /// A provider that always answers `response`.
    #[must_use]
    pub fn new(model: &str, response: &str) -> Self {
        Self {
            model: model.to_owned(),
            response: response.to_owned(),
        }
    }

    /// A provider that answers with no findings.
    #[must_use]
    pub fn silent() -> Self {
        Self::new("replay/silent", r#"{"findings":[]}"#)
    }
}

impl Provider for Replay {
    fn model(&self) -> &str {
        &self.model
    }

    fn complete(&self, _prompt: &str) -> Result<String, ProviderError> {
        Ok(self.response.clone())
    }
}

#[cfg(feature = "anthropic")]
pub use anthropic::Anthropic;

#[cfg(feature = "anthropic")]
mod anthropic {
    use super::{Provider, ProviderError};

    /// The Anthropic Messages API.
    ///
    /// Compiled only under the `anthropic` feature, so a default build of
    /// `skillmap` contains no HTTP client at all and invariant 9 holds by
    /// construction rather than by flag-checking.
    #[derive(Debug, Clone)]
    pub struct Anthropic {
        model: String,
        api_key: String,
        max_tokens: u32,
    }

    impl Anthropic {
        /// Read the key from `ANTHROPIC_API_KEY`.
        ///
        /// From the environment and never from a flag, a file this tool writes,
        /// or a prompt. A key on a command line lands in shell history and in
        /// every CI log that echoes its own commands.
        ///
        /// # Errors
        ///
        /// [`ProviderError::Call`] when the variable is unset.
        pub fn from_env(model: &str) -> Result<Self, ProviderError> {
            let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                ProviderError::Call(
                    "ANTHROPIC_API_KEY is not set. The semantic pass is the only \
                     part of skillmap that makes a network call, and it will not \
                     guess at a credential."
                        .to_owned(),
                )
            })?;
            Ok(Self {
                model: model.to_owned(),
                api_key,
                max_tokens: 4096,
            })
        }
    }

    impl Provider for Anthropic {
        fn model(&self) -> &str {
            &self.model
        }

        fn complete(&self, prompt: &str) -> Result<String, ProviderError> {
            // No `tools`, no `tool_choice`, and a single user turn. The request
            // body is as small as the API allows, because every field is one
            // more thing a compromised response could be trying to reach.
            let body = serde_json::json!({
                "model": self.model,
                "max_tokens": self.max_tokens,
                // Deterministic sampling, so the variance this layer reports is
                // the model's own and not the sampler's. It does not make the
                // call reproducible — nothing does — which is why variance is
                // measured rather than assumed away.
                "temperature": 0,
                "messages": [{ "role": "user", "content": prompt }],
            });

            // `send_string` and `serde_json`, rather than ureq's `json` feature.
            // The feature pulls its own serde integration, and this crate already
            // has serde_json — `skillmap-corpus` uses ureq the same way, so the
            // whole workspace resolves one ureq with one feature set. A
            // supply-chain auditor adding transitive crates for a convenience
            // method would be a poor trade (SECURITY.md).
            let encoded = serde_json::to_string(&body).map_err(|error| {
                ProviderError::Call(format!("cannot encode the request: {error}"))
            })?;

            let response = ureq::post("https://api.anthropic.com/v1/messages")
                .set("x-api-key", &self.api_key)
                .set("anthropic-version", "2023-06-01")
                .set("content-type", "application/json")
                .send_string(&encoded)
                .map_err(|error| ProviderError::Call(format!("{error}")))?;

            let text = response
                .into_string()
                .map_err(|error| ProviderError::Call(format!("cannot read response: {error}")))?;
            let parsed: serde_json::Value = serde_json::from_str(&text)
                .map_err(|error| ProviderError::Call(format!("response is not JSON: {error}")))?;

            // One text block, or nothing. Concatenating every block would let a
            // response smuggle a second document past the validator's idea of
            // where the JSON starts.
            parsed
                .get("content")
                .and_then(|content| content.get(0))
                .and_then(|block| block.get("text"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ProviderError::Call("response carried no text block".to_owned()))
        }
    }
}

/// The provider used when the feature is off.
///
/// Returns [`ProviderError::NotCompiledIn`] rather than silently reporting no
/// findings. "Could not run" and "ran and found nothing" are different claims,
/// and this crate exists downstream of a project that treats confusing them as
/// its defining failure (invariant 3).
#[derive(Debug, Clone, Copy, Default)]
pub struct Unavailable;

impl Provider for Unavailable {
    fn model(&self) -> &str {
        "none"
    }

    fn complete(&self, _prompt: &str) -> Result<String, ProviderError> {
        Err(ProviderError::NotCompiledIn("anthropic"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unavailable_provider_errors_rather_than_reporting_nothing() {
        // The whole point. A provider that answered `{"findings":[]}` when it
        // could not run would report every bundle clean, in the direction that
        // looks like good news.
        assert!(matches!(
            Unavailable.complete("anything"),
            Err(ProviderError::NotCompiledIn(_))
        ));
    }

    #[test]
    fn the_error_says_how_to_fix_it() {
        let message = ProviderError::NotCompiledIn("anthropic").to_string();
        assert!(message.contains("--features anthropic"), "{message}");
    }
}
