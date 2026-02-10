use alloc::string::String;
use core::fmt;

use miden_protocol::{Felt, FieldElement, Word};
use thiserror::Error;

// TOKEN LOGO URI
// ================================================================================================

/// Maximum number of UTF-8 bytes that can be stored in a [`TokenLogoURI`].
///
/// A `TokenLogoURI` is encoded into 8 words (32 felts). Each felt stores 4 bytes via
/// `u32::from_le_bytes`, giving 32 × 4 = 128 bytes of capacity.
const MAX_TOKEN_LOGO_URI_BYTES: usize = 128;

/// Number of words used to store a [`TokenLogoURI`].
const TOKEN_LOGO_URI_WORD_COUNT: usize = 8;

/// A token logo URI encoded as `[Word; 8]` (32 felts, up to 128 bytes of UTF-8).
///
/// ## Encoding
///
/// The UTF-8 string is zero-padded to [`MAX_TOKEN_LOGO_URI_BYTES`] bytes, then each consecutive
/// group of 4 bytes is packed into a single [`Felt`] via `u32::from_le_bytes`. The resulting
/// 32 felts fill eight [`Word`]s.
///
/// ## Decoding
///
/// Each felt is converted back to 4 bytes via `u32::to_le_bytes`, the trailing zero bytes are
/// trimmed, and the result is validated as UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenLogoURI([Word; TOKEN_LOGO_URI_WORD_COUNT]);

/// Errors that can occur when creating a [`TokenLogoURI`].
#[derive(Debug, Error)]
pub enum TokenLogoURIError {
    #[error(
        "token logo URI exceeds maximum length of {MAX_TOKEN_LOGO_URI_BYTES} bytes (got {0} bytes)"
    )]
    TooLong(usize),
    #[error("token logo URI contains invalid UTF-8")]
    InvalidUtf8,
    #[error("felt value {0} exceeds u32::MAX and cannot represent 4 UTF-8 bytes")]
    FeltOutOfRange(u64),
}

impl TokenLogoURI {
    /// Creates a new [`TokenLogoURI`] from a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string exceeds [`MAX_TOKEN_LOGO_URI_BYTES`] bytes.
    pub fn new(uri: &str) -> Result<Self, TokenLogoURIError> {
        let bytes = uri.as_bytes();
        if bytes.len() > MAX_TOKEN_LOGO_URI_BYTES {
            return Err(TokenLogoURIError::TooLong(bytes.len()));
        }

        // Zero-pad to MAX_TOKEN_LOGO_URI_BYTES.
        let mut padded = [0u8; MAX_TOKEN_LOGO_URI_BYTES];
        padded[..bytes.len()].copy_from_slice(bytes);

        // Pack every 4 bytes into a felt (32 felts total).
        let mut felts = [Felt::ZERO; 32];
        for (i, chunk) in padded.chunks_exact(4).enumerate() {
            let val = u32::from_le_bytes(chunk.try_into().expect("chunk is exactly 4 bytes"));
            felts[i] = Felt::new(val as u64);
        }

        let mut words = [Word::default(); TOKEN_LOGO_URI_WORD_COUNT];
        for (i, word) in words.iter_mut().enumerate() {
            *word = Word::new([
                felts[i * 4],
                felts[i * 4 + 1],
                felts[i * 4 + 2],
                felts[i * 4 + 3],
            ]);
        }

        Ok(Self(words))
    }

    /// Returns the eight words that make up this token logo URI.
    pub fn words(&self) -> &[Word; TOKEN_LOGO_URI_WORD_COUNT] {
        &self.0
    }

    /// Decodes the token logo URI back to a UTF-8 string.
    fn decode(&self) -> Result<String, TokenLogoURIError> {
        let mut bytes = [0u8; MAX_TOKEN_LOGO_URI_BYTES];
        let felts = self.as_felts();

        for (i, &felt) in felts.iter().enumerate() {
            let val = felt.as_int();
            if val > u32::MAX as u64 {
                return Err(TokenLogoURIError::FeltOutOfRange(val));
            }
            let chunk = (val as u32).to_le_bytes();
            bytes[i * 4..i * 4 + 4].copy_from_slice(&chunk);
        }

        // Trim trailing zeros and validate UTF-8.
        let len = bytes.iter().rposition(|&b| b != 0).map_or(0, |pos| pos + 1);
        String::from_utf8(bytes[..len].to_vec()).map_err(|_| TokenLogoURIError::InvalidUtf8)
    }

    /// Returns a flat array of the 32 felts.
    fn as_felts(&self) -> [Felt; 32] {
        let mut felts = [Felt::ZERO; 32];
        for (i, word) in self.0.iter().enumerate() {
            felts[i * 4] = word[0];
            felts[i * 4 + 1] = word[1];
            felts[i * 4 + 2] = word[2];
            felts[i * 4 + 3] = word[3];
        }
        felts
    }
}

impl fmt::Display for TokenLogoURI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.decode() {
            Ok(s) => f.write_str(&s),
            Err(_) => write!(f, "<invalid token logo URI>"),
        }
    }
}

impl TryFrom<[Word; TOKEN_LOGO_URI_WORD_COUNT]> for TokenLogoURI {
    type Error = TokenLogoURIError;

    fn try_from(words: [Word; TOKEN_LOGO_URI_WORD_COUNT]) -> Result<Self, Self::Error> {
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
    fn empty_uri() {
        let uri = TokenLogoURI::new("").unwrap();
        assert_eq!(uri.to_string(), "");
    }

    #[test]
    fn short_uri() {
        let uri = TokenLogoURI::new("https://example.com/logo.png").unwrap();
        assert_eq!(uri.to_string(), "https://example.com/logo.png");
    }

    #[test]
    fn max_length_uri() {
        // Exactly 128 bytes.
        let s = "x".repeat(MAX_TOKEN_LOGO_URI_BYTES);
        let uri = TokenLogoURI::new(&s).unwrap();
        assert_eq!(uri.to_string(), s);
    }

    #[test]
    fn too_long_uri() {
        let s = "y".repeat(MAX_TOKEN_LOGO_URI_BYTES + 1);
        let err = TokenLogoURI::new(&s).unwrap_err();
        assert!(matches!(err, TokenLogoURIError::TooLong(129)));
    }

    #[test]
    fn roundtrip_via_words() {
        let original = TokenLogoURI::new("https://tokens.example.org/eth-logo.svg").unwrap();
        let words = *original.words();
        let restored = TokenLogoURI::try_from(words).unwrap();
        assert_eq!(original, restored);
        assert_eq!(restored.to_string(), "https://tokens.example.org/eth-logo.svg");
    }

    #[test]
    fn try_from_invalid_felt_out_of_range() {
        let mut words = [Word::default(); TOKEN_LOGO_URI_WORD_COUNT];
        words[0] = Word::new([Felt::new(u64::MAX / 2), Felt::ZERO, Felt::ZERO, Felt::ZERO]);
        let err = TokenLogoURI::try_from(words).unwrap_err();
        assert!(matches!(err, TokenLogoURIError::FeltOutOfRange(_)));
    }
}
