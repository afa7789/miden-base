use alloc::string::String;

use miden_protocol::account::StorageSlotName;
use miden_protocol::errors::{AccountError, TokenSymbolError};
use thiserror::Error;

mod basic_fungible;
mod basic_non_fungible;
mod network_fungible;
mod network_non_fungible;
mod nft_metadata;
mod token_metadata;

pub use basic_fungible::{BasicFungibleFaucet, create_basic_fungible_faucet};
pub use basic_non_fungible::{BasicNonFungibleFaucet, create_basic_non_fungible_faucet};
pub use network_fungible::{NetworkFungibleFaucet, create_network_fungible_faucet};
pub use network_non_fungible::{NetworkNonFungibleFaucet, create_network_non_fungible_faucet};
pub use nft_metadata::NftMetadata;
pub use token_metadata::TokenMetadata;

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
    #[error("storage slot name mismatch: expected {expected}, got {actual}")]
    SlotNameMismatch {
        expected: StorageSlotName,
        actual: StorageSlotName,
    },
    #[error("unsupported authentication method: {0}")]
    UnsupportedAuthMethod(String),
    #[error("account creation failed")]
    AccountError(#[source] AccountError),
    #[error("account is not a fungible faucet account")]
    NotAFungibleFaucetAccount,
}

// NON-FUNGIBLE FAUCET ERROR
// ================================================================================================

/// Non-fungible faucet related errors.
#[derive(Debug, Error)]
pub enum NonFungibleFaucetError {
    #[error("max supply cannot be zero")]
    MaxSupplyCannotBeZero,
    #[error("current supply {current_supply} exceeds max supply {max_supply}")]
    SupplyExceedsMaxSupply { current_supply: u64, max_supply: u64 },
    #[error(
        "account interface does not have the procedures of the basic non-fungible faucet component"
    )]
    MissingBasicNonFungibleFaucetInterface,
    #[error(
        "account interface does not have the procedures of the network non-fungible faucet component"
    )]
    MissingNetworkNonFungibleFaucetInterface,
    #[error("failed to retrieve storage slot with name {slot_name}")]
    StorageLookupFailed {
        slot_name: StorageSlotName,
        source: AccountError,
    },
    #[error("invalid token symbol")]
    InvalidTokenSymbol(#[source] TokenSymbolError),
    #[error("storage slot name mismatch: expected {expected}, got {actual}")]
    SlotNameMismatch {
        expected: StorageSlotName,
        actual: StorageSlotName,
    },
    #[error("unsupported authentication method: {0}")]
    UnsupportedAuthMethod(String),
    #[error("account creation failed")]
    AccountError(#[source] AccountError),
    #[error("account is not a non-fungible faucet account")]
    NotANonFungibleFaucetAccount,
}
