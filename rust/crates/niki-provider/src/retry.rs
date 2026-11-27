//! Retry policy for provider requests.
//!
//! A transient 429, 5xx or connection failure should not end a turn, but a
//! retry is only safe before any event reaches the caller: once a frame has
//! been parsed and yielded, a second attempt would duplicate text. Only the
//! request setup and the response headers are retried here; the body is
//! streamed exactly once.
//!
//! The delay calculation and the attempt policy are pure functions, and the
//! attempt loop takes its sleep as a parameter, so tests can record the waits
//! instead of taking them.

use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::ProviderError;

/// Total attempts, including the first.
pub const MAX_ATTEMPTS: u32 = 3;

/// First backoff, before jitter.
pub const BASE_DELAY_MS: u64 = 300;

/// Longest backoff between attempts.
pub const MAX_DELAY_MS: u64 = 2_000;

/// Longest `Retry-After` the client honours, so a header cannot park a turn for
/// minutes.
pub const RETRY_AFTER_CAP_MS: u64 = 10_000;

/// Jitter applied to each backoff, as a percentage of the nominal delay.
pub const JITTER_PERCENT: u64 = 20;

/// Retry behaviour for a provider request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts, including the first. Clamped to [`MAX_ATTEMPTS`].
    pub attempts: u32,
    /// First backoff, tripled per attempt and capped at [`MAX_DELAY_MS`].
    pub base_delay_ms: u64,
}

impl RetryPolicy {
    /// The default: three attempts starting at 300 ms.
    #[must_use]
    pub const fn default_policy() -> Self {
        Self {
            attempts: MAX_ATTEMPTS,
            base_delay_ms: BASE_DELAY_MS,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/// Whether a response status is worth another attempt.
///
/// 408, 409 and 425 mean the request did not complete; 429 means slow down;
/// 500, 502, 503 and 504 are the transient server failures. Everything else,
/// including 501, is the provider's permanent answer.
#[must_use]
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504)
}

/// Whether another attempt could succeed for `error`.
///
/// A connection failure or timeout is always worth another attempt; a decode
/// failure or a missing key is not.
#[must_use]
pub fn is_retryable(error: &ProviderError) -> bool {
    match error {
        ProviderError::Http { status, .. } => is_retryable_status(*status),
        ProviderError::Transport { .. } => true,
        ProviderError::Decode { .. } | ProviderError::MissingKey { .. } => false,
    }
}

/// Whether another attempt may follow `attempts_made` attempts.
#[must_use]
pub fn may_retry(policy: RetryPolicy, attempts_made: u32, error: &ProviderError) -> bool {
    attempts_made < policy.attempts.clamp(1, MAX_ATTEMPTS) && is_retryable(error)
}

/// Unjittered wait before retry `retry` (zero-based), tripling each time:
/// `base`, `3 * base`, `9 * base`, and so on.
#[must_use]
pub fn nominal_delay_ms(base_delay_ms: u64, retry: u32) -> u64 {
    base_delay_ms.saturating_mul(3u64.saturating_pow(retry))
}

/// `nominal_ms` spread by ±20%, where `sample` 0.5 leaves it unchanged and the
/// ends of the range are -20% and +20%.
#[must_use]
pub fn jittered_delay_ms(nominal_ms: u64, sample: f64) -> u64 {
    let offset = (sample.clamp(0.0, 1.0) - 0.5) * 2.0 * (JITTER_PERCENT as f64 / 100.0);
    let scaled = (nominal_ms as f64 * (1.0 + offset)).round();
    scaled.max(0.0) as u64
}

/// Backoff before retry `retry`, jittered by `sample` and capped.
#[must_use]
pub fn backoff_ms(policy: RetryPolicy, retry: u32, sample: f64) -> u64 {
    jittered_delay_ms(nominal_delay_ms(policy.base_delay_ms, retry), sample).min(MAX_DELAY_MS)
}

/// Parse a `Retry-After` header value as a whole number of seconds.
///
/// An HTTP-date is not a number, so it is ignored and the caller falls back to
/// the exponential backoff. The result is capped at [`RETRY_AFTER_CAP_MS`].
#[must_use]
pub fn retry_after_ms(value: &str) -> Option<u64> {
    let seconds: u64 = value.trim().parse().ok()?;
    Some(seconds.saturating_mul(1_000).min(RETRY_AFTER_CAP_MS))
}

/// One failed attempt, with the server's requested wait when it sent one.
pub(crate) struct Failure {
    pub(crate) error: ProviderError,
    pub(crate) retry_after_ms: Option<u64>,
}

impl Failure {
    pub(crate) const fn new(error: ProviderError, retry_after_ms: Option<u64>) -> Self {
        Self {
            error,
            retry_after_ms,
        }
    }

    /// A failure that is never retried (request construction, say).
    pub(crate) const fn fatal(error: ProviderError) -> Self {
        Self::new(error, None)
    }
}

/// Run `attempt` until it succeeds, fails permanently, or the policy runs out.
///
/// `sleep` is called between attempts: `tokio::time::sleep` in production, a
/// recorder in tests.
pub(crate) async fn with_retries<T, F, Fut, S, SFut>(
    policy: RetryPolicy,
    mut attempt: F,
    mut sleep: S,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Failure>>,
    S: FnMut(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let mut made = 1;
    loop {
        match attempt().await {
            Ok(value) => return Ok(value),
            Err(failure) => {
                if !may_retry(policy, made, &failure.error) {
                    return Err(failure.error);
                }
                let delay = failure
                    .retry_after_ms
                    .unwrap_or_else(|| backoff_ms(policy, made - 1, jitter_sample()));
                sleep(Duration::from_millis(delay)).await;
                made += 1;
            }
        }
    }
}

fn jitter_sample() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    f64::from(nanos % 10_000) / 10_000.0
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::future::ready;

    use super::*;

    fn http(status: u16) -> ProviderError {
        ProviderError::Http {
            provider: "mock".to_owned(),
            status,
            body: String::new(),
        }
    }

    #[test]
    fn statuses_are_classified() {
        for status in [408, 409, 425, 429, 500, 502, 503, 504] {
            assert!(is_retryable_status(status), "{status} should be retryable");
        }
        for status in [400, 401, 403, 404, 422, 501] {
            assert!(
                !is_retryable_status(status),
                "{status} should not be retryable"
            );
        }
        assert!(is_retryable(&ProviderError::Transport {
            provider: "mock".to_owned(),
            detail: "connection reset".to_owned(),
        }));
        assert!(!is_retryable(&ProviderError::Decode {
            provider: "mock".to_owned(),
            detail: "bad frame".to_owned(),
        }));
    }

    #[test]
    fn attempts_stop_at_three() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.attempts, MAX_ATTEMPTS);
        let error = http(503);
        assert!(may_retry(policy, 1, &error));
        assert!(may_retry(policy, 2, &error));
        assert!(!may_retry(policy, 3, &error));
        assert!(!may_retry(policy, 1, &http(400)));
        assert!(!may_retry(
            RetryPolicy {
                attempts: 9,
                base_delay_ms: BASE_DELAY_MS,
            },
            3,
            &error
        ));
    }

    #[test]
    fn backoff_triples_and_caps() {
        let policy = RetryPolicy::default();
        assert_eq!(nominal_delay_ms(policy.base_delay_ms, 0), 300);
        assert_eq!(nominal_delay_ms(policy.base_delay_ms, 1), 900);
        assert_eq!(nominal_delay_ms(policy.base_delay_ms, 2), 2_700);

        assert_eq!(backoff_ms(policy, 0, 0.5), 300);
        assert_eq!(backoff_ms(policy, 1, 0.5), 900);
        assert_eq!(backoff_ms(policy, 0, 0.0), 240);
        assert_eq!(backoff_ms(policy, 0, 1.0), 360);
        assert_eq!(backoff_ms(policy, 2, 0.0), MAX_DELAY_MS);
        assert_eq!(backoff_ms(policy, 2, 0.5), MAX_DELAY_MS);
    }

    #[test]
    fn retry_after_is_seconds_capped_and_ignores_dates() {
        assert_eq!(retry_after_ms("3"), Some(3_000));
        assert_eq!(retry_after_ms(" 0 "), Some(0));
        assert_eq!(retry_after_ms("10"), Some(RETRY_AFTER_CAP_MS));
        assert_eq!(retry_after_ms("120"), Some(RETRY_AFTER_CAP_MS));
        assert_eq!(retry_after_ms("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(retry_after_ms("1.5"), None);
        assert_eq!(retry_after_ms(""), None);
    }

    #[tokio::test]
    async fn loop_stops_after_three_attempts_and_returns_the_last_status() {
        let calls = Cell::new(0);
        let waits = RefCell::new(Vec::new());
        let error = with_retries(
            RetryPolicy::default(),
            || {
                calls.set(calls.get() + 1);
                ready(Err::<(), _>(Failure::fatal(http(503))))
            },
            |delay| {
                waits.borrow_mut().push(delay);
                ready(())
            },
        )
        .await
        .expect_err("every attempt failed");

        assert_eq!(calls.get(), 3);
        assert_eq!(waits.borrow().len(), 2);
        assert!(matches!(error, ProviderError::Http { status: 503, .. }));

        let waits = waits.borrow();
        let first = waits[0].as_millis();
        let second = waits[1].as_millis();
        assert!((240..=360).contains(&first), "first wait was {first} ms");
        assert!(
            (720..=1_080).contains(&second),
            "second wait was {second} ms"
        );
    }

    #[tokio::test]
    async fn loop_does_not_wait_for_a_permanent_failure() {
        let calls = Cell::new(0);
        let waits = RefCell::new(Vec::new());
        let error = with_retries(
            RetryPolicy::default(),
            || {
                calls.set(calls.get() + 1);
                ready(Err::<(), _>(Failure::fatal(http(400))))
            },
            |delay| {
                waits.borrow_mut().push(delay);
                ready(())
            },
        )
        .await
        .expect_err("the request was rejected");

        assert_eq!(calls.get(), 1);
        assert!(waits.borrow().is_empty());
        assert!(matches!(error, ProviderError::Http { status: 400, .. }));
    }

    #[tokio::test]
    async fn loop_honours_retry_after() {
        let calls = Cell::new(0);
        let waits = RefCell::new(Vec::new());
        with_retries(
            RetryPolicy::default(),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    ready(Err::<(), _>(Failure::new(http(429), Some(0))))
                } else {
                    ready(Ok(()))
                }
            },
            |delay| {
                waits.borrow_mut().push(delay);
                ready(())
            },
        )
        .await
        .expect("the retry succeeds");

        assert_eq!(calls.get(), 2);
        assert_eq!(waits.borrow().as_slice(), [Duration::ZERO]);
    }
}
