//! Every time-varying glyph, in one place.
//!
//! The event loop wakes once per [`LOADING_INTERVAL`] while a turn runs, and
//! [`crate::App::tick`] advances one counter. Each effect turns that counter
//! into a frame here, so the schedule is one table rather than a constant in
//! every call site. Reduced motion replaces movement with a static glyph and
//! never removes the information that something is happening.

use std::time::Duration;

/// How often the loop wakes to advance the motion counter while a turn runs.
///
/// One visible cycle per second: slow enough to read, cheap enough that a long
/// turn does not cost more than ten redraws a second.
pub const LOADING_INTERVAL: Duration = Duration::from_millis(100);

/// Ten braille frames, one visible cycle per second.
pub const UNICODE_LOADING: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Four frames for a terminal that cannot draw braille.
pub const ASCII_LOADING: [char; 4] = ['|', '/', '-', '\\'];

/// The static stand-in when motion is reduced, so activity stays visible.
pub const RESTING_UNICODE: char = '◌';

/// The ASCII stand-in for [`RESTING_UNICODE`].
pub const RESTING_ASCII: char = 'o';

/// Ticks between the two activity frames: six at 100 ms is 600 ms.
pub const ACTIVITY_TICKS: usize = 6;

/// The running-tool marker, lit and dimmed.
pub const ACTIVITY_UNICODE: [char; 2] = ['•', '◦'];

/// The ASCII running-tool marker, lit and dimmed.
pub const ACTIVITY_ASCII: [char; 2] = ['*', 'o'];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interval_matches_the_frame_count() {
        // Ten frames at 100 ms is one cycle per second, and the activity pair
        // is five times slower than the spinner.
        assert_eq!(UNICODE_LOADING.len(), 10);
        assert_eq!(LOADING_INTERVAL.as_millis() * 10, 1_000);
        assert_eq!(LOADING_INTERVAL.as_millis() as usize * ACTIVITY_TICKS, 600);
    }
}
