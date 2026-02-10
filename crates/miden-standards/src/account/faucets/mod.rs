use alloc::string::String;

use miden_protocol::account::StorageSlotName;
use miden_protocol::errors::{AccountError, TokenSymbolError};
use miden_protocol::utils::sync::LazyLock;
use miden_protocol::{Felt, FieldElement, Word};
use thiserror::Error;

mod basic_fungible;
mod network_fungible;
mod token_logo_uri;
mod token_name;

pub use basic_fungible::{BasicFungibleFaucet, create_basic_fungible_faucet};
pub use network_fungible::{NetworkFungibleFaucet, create_network_fungible_faucet};
pub use token_logo_uri::{TokenLogoURI, TokenLogoURIError};
pub use token_name::{TokenName, TokenNameError};

static METADATA_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::fungible_faucets::metadata")
        .expect("storage slot name should be valid")
});

/// Index of the core metadata double-word in the metadata map (double_word_array layout).
/// Word0: [token_supply, max_supply, decimals, token_symbol]
/// Word1: [reserved, reserved, reserved, reserved]
pub const METADATA_DOUBLE_WORD_INDEX: u64 = 0;

/// Index of the token name double-word in the metadata map.
/// Stores a [`TokenName`] (2 words = 8 felts = 32 bytes of UTF-8).
pub const TOKEN_NAME_DOUBLE_WORD_INDEX: u64 = 1;

/// Starting index for the token logo URI double-words in the metadata map.
/// A [`TokenLogoURI`] uses 8 words = 4 consecutive double-word indices (2–5).
pub const TOKEN_LOGO_URI_DOUBLE_WORD_INDEX_START: u64 = 2;

/// Returns the two map keys for a double-word at the given `index` in the double_word_array
/// layout.
///
/// - Key for word0: `[index, 0, 0, 0]`
/// - Key for word1: `[index, 1, 0, 0]`
pub fn double_word_map_keys(index: u64) -> (Word, Word) {
    let key0 = Word::new([Felt::new(index), Felt::ZERO, Felt::ZERO, Felt::ZERO]);
    let key1 = Word::new([Felt::new(index), Felt::ONE, Felt::ZERO, Felt::ZERO]);
    (key0, key1)
}

/// Map key for the first word of the metadata double-word: key = [index, 0, 0, 0].
pub fn metadata_map_key_word0() -> Word {
    let (key0, _) = double_word_map_keys(METADATA_DOUBLE_WORD_INDEX);
    key0
}

/// Map key for the second word of the metadata double-word: key = [index, 1, 0, 0].
pub fn metadata_map_key_word1() -> Word {
    let (_, key1) = double_word_map_keys(METADATA_DOUBLE_WORD_INDEX);
    key1
}

// FUNGIBLE FAUCET ERROR
// ================================================================================================

/// Basic fungible faucet related errors.
#[derive(Debug, Error)]
pub enum FungibleFaucetError {
    #[error("faucet metadata decimals is {actual} which exceeds max value of {max}")]
    TooManyDecimals { actual: u64, max: u8 },
    #[error("faucet metadata max supply is {actual} which exceeds max value of {max}")]
    MaxSupplyTooLarge { actual: u64, max: u64 },
    #[error("token supply {token_supply} exceeds max_supply {max_supply}")]
    TokenSupplyExceedsMaxSupply { token_supply: u64, max_supply: u64 },
    #[error(
        "account interface does not have the procedures of the basic fungible faucet component"
    )]
    MissingBasicFungibleFaucetInterface,
    #[error(
        "account interface does not have the procedures of the network fungible faucet component"
    )]
    MissingNetworkFungibleFaucetInterface,
    #[error("failed to retrieve storage slot with name {slot_name}")]
    StorageLookupFailed {
        slot_name: StorageSlotName,
        source: AccountError,
    },
    #[error("invalid token symbol")]
    InvalidTokenSymbol(#[source] TokenSymbolError),
    #[error("invalid token name")]
    InvalidTokenName(#[source] TokenNameError),
    #[error("invalid token logo URI")]
    InvalidTokenLogoURI(#[source] TokenLogoURIError),
    #[error("unsupported authentication scheme: {0}")]
    UnsupportedAuthScheme(String),
    #[error("account creation failed")]
    AccountError(#[source] AccountError),
    #[error("account is not a fungible faucet account")]
    NotAFungibleFaucetAccount,
}

#[cfg(test)]
mod tests {
    use miden_protocol::{Felt, FieldElement, Word};

    use super::{
        METADATA_DOUBLE_WORD_INDEX, TOKEN_LOGO_URI_DOUBLE_WORD_INDEX_START,
        TOKEN_NAME_DOUBLE_WORD_INDEX, double_word_map_keys, metadata_map_key_word0,
        metadata_map_key_word1,
    };

    #[test]
    fn metadata_map_keys_match_double_word_array_layout() {
        assert_eq!(
            metadata_map_key_word0(),
            Word::new([
                Felt::new(METADATA_DOUBLE_WORD_INDEX),
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
            ])
        );
        assert_eq!(
            metadata_map_key_word1(),
            Word::new([
                Felt::new(METADATA_DOUBLE_WORD_INDEX),
                Felt::ONE,
                Felt::ZERO,
                Felt::ZERO,
            ])
        );
    }

    #[test]
    fn double_word_map_keys_are_correct() {
        let (key0, key1) = double_word_map_keys(TOKEN_NAME_DOUBLE_WORD_INDEX);
        assert_eq!(
            key0,
            Word::new([Felt::new(1), Felt::ZERO, Felt::ZERO, Felt::ZERO])
        );
        assert_eq!(
            key1,
            Word::new([Felt::new(1), Felt::ONE, Felt::ZERO, Felt::ZERO])
        );

        let (key0, key1) = double_word_map_keys(TOKEN_LOGO_URI_DOUBLE_WORD_INDEX_START);
        assert_eq!(
            key0,
            Word::new([Felt::new(2), Felt::ZERO, Felt::ZERO, Felt::ZERO])
        );
        assert_eq!(
            key1,
            Word::new([Felt::new(2), Felt::ONE, Felt::ZERO, Felt::ZERO])
        );
    }
}
