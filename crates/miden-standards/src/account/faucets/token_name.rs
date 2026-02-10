use alloc::string::String;
use core::fmt;

use miden_protocol::{Felt, FieldElement, Word};
use thiserror::Error;

// TOKEN NAME
// ================================================================================================

/// Maximum number of UTF-8 bytes that can be stored in a [`TokenName`].
///
/// A `TokenName` is encoded into 2 words (8 felts). Each felt stores 4 bytes via
/// `u32::from_le_bytes`, giving 8 × 4 = 32 bytes of capacity.
const MAX_TOKEN_NAME_BYTES: usize = 32;

/// A token name encoded as `[Word; 2]` (8 felts, up to 32 bytes of UTF-8).
///
/// ## Encoding
///
/// The UTF-8 string is zero-padded to [`MAX_TOKEN_NAME_BYTES`] bytes, then each consecutive
/// group of 4 bytes is packed into a single [`Felt`] via `u32::from_le_bytes`. The resulting
/// 8 felts fill two [`Word`]s.
///
/// ## Decoding
///
/// Each felt is converted back to 4 bytes via `u32::to_le_bytes`, the trailing zero bytes are
/// trimmed, and the result is validated as UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenName([Word; 2]);

/// Errors that can occur when creating a [`TokenName`].
#[derive(Debug, Error)]
pub enum TokenNameError {
    #[error("token name exceeds maximum length of {MAX_TOKEN_NAME_BYTES} bytes (got {0} bytes)")]
    TooLong(usize),
    #[error("token name contains invalid UTF-8")]
    InvalidUtf8,
    #[error("felt value {0} exceeds u32::MAX and cannot represent 4 UTF-8 bytes")]
    FeltOutOfRange(u64),
}

impl TokenName {
    /// Creates a new [`TokenName`] from a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string exceeds [`MAX_TOKEN_NAME_BYTES`] bytes.
    pub fn new(name: &str) -> Result<Self, TokenNameError> {
        let bytes = name.as_bytes();
        if bytes.len() > MAX_TOKEN_NAME_BYTES {
            return Err(TokenNameError::TooLong(bytes.len()));
        }

        // Zero-pad to MAX_TOKEN_NAME_BYTES.
        let mut padded = [0u8; MAX_TOKEN_NAME_BYTES];
        padded[..bytes.len()].copy_from_slice(bytes);

        // Pack every 4 bytes into a felt.
        let mut felts = [Felt::ZERO; 8];
        for (i, chunk) in padded.chunks_exact(4).enumerate() {
            let val = u32::from_le_bytes(chunk.try_into().expect("chunk is exactly 4 bytes"));
            felts[i] = Felt::new(val as u64);
        }

        Ok(Self([
            Word::new([felts[0], felts[1], felts[2], felts[3]]),
            Word::new([felts[4], felts[5], felts[6], felts[7]]),
        ]))
    }

    /// Returns the two words that make up this token name.
    pub fn words(&self) -> &[Word; 2] {
        &self.0
    }

    /// Decodes the token name back to a UTF-8 string.
    fn decode(&self) -> Result<String, TokenNameError> {
        let mut bytes = [0u8; MAX_TOKEN_NAME_BYTES];
        let felts = self.as_felts();

        for (i, &felt) in felts.iter().enumerate() {
            let val = felt.as_int();
            if val > u32::MAX as u64 {
                return Err(TokenNameError::FeltOutOfRange(val));
            }
            let chunk = (val as u32).to_le_bytes();
            bytes[i * 4..i * 4 + 4].copy_from_slice(&chunk);
        }

        // Trim trailing zeros and validate UTF-8.
        let len = bytes.iter().rposition(|&b| b != 0).map_or(0, |pos| pos + 1);
        String::from_utf8(bytes[..len].to_vec()).map_err(|_| TokenNameError::InvalidUtf8)
    }

    /// Returns a flat array of the 8 felts.
    fn as_felts(&self) -> [Felt; 8] {
        let [w0, w1] = self.0;
        [w0[0], w0[1], w0[2], w0[3], w1[0], w1[1], w1[2], w1[3]]
    }
}

impl fmt::Display for TokenName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.decode() {
            Ok(s) => f.write_str(&s),
            Err(_) => write!(f, "<invalid token name>"),
        }
    }
}

impl TryFrom<[Word; 2]> for TokenName {
    type Error = TokenNameError;

    fn try_from(words: [Word; 2]) -> Result<Self, Self::Error> {
        let candidate = Self(words);
        // Validate that all felts are in range and the result is valid UTF-8.
        candidate.decode()?;
        Ok(candidate)
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn empty_name() {
        let name = TokenName::new("").unwrap();
        assert_eq!(name.to_string(), "");
        assert_eq!(name.words(), &[Word::default(), Word::default()]);
    }

    #[test]
    fn short_ascii_name() {
        let name = TokenName::new("ETH").unwrap();
        assert_eq!(name.to_string(), "ETH");
    }

    #[test]
    fn max_length_name() {
        // Exactly 32 bytes.
        let s = "A".repeat(MAX_TOKEN_NAME_BYTES);
        let name = TokenName::new(&s).unwrap();
        assert_eq!(name.to_string(), s);
    }

    #[test]
    fn too_long_name() {
        let s = "B".repeat(MAX_TOKEN_NAME_BYTES + 1);
        let err = TokenName::new(&s).unwrap_err();
        assert!(matches!(err, TokenNameError::TooLong(33)));
    }

    #[test]
    fn unicode_name() {
        // "Tökën" is 7 bytes in UTF-8 (ö = 2 bytes, ë = 2 bytes).
        let name = TokenName::new("Tökën").unwrap();
        assert_eq!(name.to_string(), "Tökën");
    }

    #[test]
    fn roundtrip_via_words() {
        let original = TokenName::new("Polygon").unwrap();
        let words = *original.words();
        let restored = TokenName::try_from(words).unwrap();
        assert_eq!(original, restored);
        assert_eq!(restored.to_string(), "Polygon");
    }

    #[test]
    fn try_from_invalid_felt_out_of_range() {
        let mut words = [Word::default(); 2];
        // Put a value larger than u32::MAX in the first felt.
        words[0] = Word::new([Felt::new(u64::MAX / 2), Felt::ZERO, Felt::ZERO, Felt::ZERO]);
        let err = TokenName::try_from(words).unwrap_err();
        assert!(matches!(err, TokenNameError::FeltOutOfRange(_)));
    }
}
