use miden_protocol::account::{AccountStorage, StorageSlot, StorageSlotName};
use miden_protocol::asset::TokenSymbol;
use miden_protocol::utils::sync::LazyLock;
use miden_protocol::{Felt, Word};

use super::NonFungibleFaucetError;

// CONSTANTS
// ================================================================================================

static NFT_METADATA_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::nft::metadata")
        .expect("storage slot name should be valid")
});

// NFT METADATA
// ================================================================================================

/// Token metadata for non-fungible faucet accounts.
///
/// This struct encapsulates the metadata associated with an NFT faucet:
/// - `current_supply`: The number of currently active (non-burned) NFTs.
/// - `max_supply`: The maximum number of tokens in the collection.
/// - `next_token_id`: Auto-incrementing counter, never decreases (even after burns).
/// - `symbol`: The token symbol (ASCII, up to 8 chars).
///
/// The metadata is stored in a single storage slot as:
/// `[current_supply, max_supply, next_token_id, symbol]`
#[derive(Debug, Clone, Copy)]
pub struct NftMetadata {
    current_supply: u64,
    max_supply: u64,
    next_token_id: u64,
    symbol: TokenSymbol,
}

impl NftMetadata {
    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Creates new [`NftMetadata`] with zero supply and zero next_token_id.
    ///
    /// # Errors
    /// Returns an error if `max_supply` is zero.
    pub fn new(symbol: TokenSymbol, max_supply: Felt) -> Result<Self, NonFungibleFaucetError> {
        let max_supply_val = max_supply.as_canonical_u64();
        if max_supply_val == 0 {
            return Err(NonFungibleFaucetError::MaxSupplyCannotBeZero);
        }

        Ok(Self {
            current_supply: 0,
            max_supply: max_supply_val,
            next_token_id: 0,
            symbol,
        })
    }

    /// Creates [`NftMetadata`] with the specified supply state.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `max_supply` is zero.
    /// - `current_supply` exceeds `max_supply`.
    pub fn with_supply(
        symbol: TokenSymbol,
        max_supply: Felt,
        current_supply: Felt,
        next_token_id: Felt,
    ) -> Result<Self, NonFungibleFaucetError> {
        let max_supply_val = max_supply.as_canonical_u64();
        let current_supply_val = current_supply.as_canonical_u64();

        if max_supply_val == 0 {
            return Err(NonFungibleFaucetError::MaxSupplyCannotBeZero);
        }

        if current_supply_val > max_supply_val {
            return Err(NonFungibleFaucetError::SupplyExceedsMaxSupply {
                current_supply: current_supply_val,
                max_supply: max_supply_val,
            });
        }

        Ok(Self {
            current_supply: current_supply_val,
            max_supply: max_supply_val,
            next_token_id: next_token_id.as_canonical_u64(),
            symbol,
        })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the [`StorageSlotName`] where the NFT metadata is stored.
    pub fn metadata_slot() -> &'static StorageSlotName {
        &NFT_METADATA_SLOT_NAME
    }

    /// Returns the number of currently active (non-burned) NFTs.
    pub fn current_supply(&self) -> u64 {
        self.current_supply
    }

    /// Returns the maximum number of tokens in the collection.
    pub fn max_supply(&self) -> Felt {
        Felt::new(self.max_supply)
    }

    /// Returns the next token ID that will be assigned on mint.
    pub fn next_token_id(&self) -> u64 {
        self.next_token_id
    }

    /// Returns the token symbol.
    pub fn symbol(&self) -> TokenSymbol {
        self.symbol
    }
}

// TRAIT IMPLEMENTATIONS
// ================================================================================================

impl TryFrom<Word> for NftMetadata {
    type Error = NonFungibleFaucetError;

    /// Parses NFT metadata from a Word.
    ///
    /// The Word is expected to be in the format: `[current_supply, max_supply, next_token_id, symbol]`
    fn try_from(word: Word) -> Result<Self, Self::Error> {
        let [current_supply, max_supply, next_token_id, token_symbol] = *word;

        let symbol =
            TokenSymbol::try_from(token_symbol).map_err(NonFungibleFaucetError::InvalidTokenSymbol)?;

        Self::with_supply(symbol, max_supply, current_supply, next_token_id)
    }
}

impl From<NftMetadata> for Word {
    fn from(meta: NftMetadata) -> Self {
        // Storage layout: [current_supply, max_supply, next_token_id, symbol]
        Word::new([
            Felt::new(meta.current_supply),
            Felt::new(meta.max_supply),
            Felt::new(meta.next_token_id),
            meta.symbol.into(),
        ])
    }
}

impl From<NftMetadata> for StorageSlot {
    fn from(meta: NftMetadata) -> Self {
        StorageSlot::with_value(NftMetadata::metadata_slot().clone(), meta.into())
    }
}

impl TryFrom<&StorageSlot> for NftMetadata {
    type Error = NonFungibleFaucetError;

    fn try_from(slot: &StorageSlot) -> Result<Self, Self::Error> {
        if slot.name() != Self::metadata_slot() {
            return Err(NonFungibleFaucetError::SlotNameMismatch {
                expected: Self::metadata_slot().clone(),
                actual: slot.name().clone(),
            });
        }
        NftMetadata::try_from(slot.value())
    }
}

impl TryFrom<&AccountStorage> for NftMetadata {
    type Error = NonFungibleFaucetError;

    fn try_from(storage: &AccountStorage) -> Result<Self, Self::Error> {
        let metadata_word = storage.get_item(NftMetadata::metadata_slot()).map_err(|err| {
            NonFungibleFaucetError::StorageLookupFailed {
                slot_name: NftMetadata::metadata_slot().clone(),
                source: err,
            }
        })?;

        NftMetadata::try_from(metadata_word)
    }
}

