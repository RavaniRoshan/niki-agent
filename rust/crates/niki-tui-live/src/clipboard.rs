//! The escape that hands text to the terminal's clipboard.
//!
//! OSC 52 lets the terminal, which owns the clipboard, take a base64 payload
//! from the application, so copying needs no platform tool and works over a
//! remote terminal.

use std::io::{self, Write};

/// Ask the terminal to take `text` as the clipboard, as one OSC 52 sequence.
///
/// # Errors
///
/// Returns the write or flush failure of `out`.
pub fn copy(out: &mut impl Write, text: &str) -> io::Result<()> {
    write!(out, "\x1b]52;c;{}\x1b\\", base64(text.as_bytes()))?;
    out.flush()
}

/// Standard base64 with padding, which is what OSC 52 carries.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = chunk.get(1).copied().map(u32::from);
        let third = chunk.get(2).copied().map(u32::from);
        let group = (first << 16) | (second.unwrap_or(0) << 8) | third.unwrap_or(0);
        out.push(character(ALPHABET, (group >> 18) & 0x3f));
        out.push(character(ALPHABET, (group >> 12) & 0x3f));
        out.push(if second.is_some() {
            character(ALPHABET, (group >> 6) & 0x3f)
        } else {
            '='
        });
        out.push(if third.is_some() {
            character(ALPHABET, group & 0x3f)
        } else {
            '='
        });
    }
    out
}

fn character(alphabet: &[u8; 64], index: u32) -> char {
    let index = usize::try_from(index).unwrap_or(0).min(alphabet.len() - 1);
    char::from(alphabet[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_the_last_group() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"abcd"), "YWJjZA==");
        assert_eq!(base64("✓".as_bytes()), "4pyT");
    }
}
