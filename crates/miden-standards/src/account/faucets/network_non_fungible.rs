use miden_protocol::account::component::{
    AccountComponentMetadata,
    FeltSchema,
    SchemaType,
    StorageSchema,
    StorageSlotSchema,
};
use miden_protocol::account::{
    Account,
    AccountBuilder,
    AccountComponent,
    AccountId,
    AccountStorage,
    AccountStorageMode,
    AccountType,
    StorageMap,
    StorageSlot,
    StorageSlotName,
};
use miden_protocol::asset::TokenSymbol;
use miden_protocol::utils::sync::LazyLock;
use miden_protocol::{Felt, Word};

use super::{NftMetadata, NonFungibleFaucetError};
use crate::account::auth::NoAuth;
use crate::account::components::network_non_fungible_faucet_library;
use crate::account::interface::{AccountComponentInterface, AccountInterface, AccountInterfaceExt};
use crate::procedure_digest;

// CONSTANTS
// ================================================================================================

const TOKEN_SYMBOL_TYPE: &str = "miden::standards::fungible_faucets::metadata::token_symbol";

static OWNER_CONFIG_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::access::ownable::owner_config")
        .expect("storage slot name should be valid")
});

static TOKEN_MAP_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::nft::tokens")
        .expect("storage slot name should be valid")
});

// NETWORK NON-FUNGIBLE FAUCET ACCOUNT COMPONENT
// ================================================================================================

procedure_digest!(
    NETWORK_NON_FUNGIBLE_FAUCET_DISTRIBUTE,
    NetworkNonFungibleFaucet::DISTRIBUTE_PROC_NAME,
    network_non_fungible_faucet_library
);

procedure_digest!(
    NETWORK_NON_FUNGIBLE_FAUCET_BURN,
    NetworkNonFungibleFaucet::BURN_PROC_NAME,
    network_non_fungible_faucet_library
);

/// An [`AccountComponent`] implementing a network non-fungible faucet.
///
/// It reexports the procedures from `miden::standards::faucets::non_fungible`. When linking
/// against this component, the `miden` library (i.e.
/// [`ProtocolLib`](miden_protocol::ProtocolLib)) must be available to the assembler which is the
/// case when using [`CodeBuilder`][builder]. The procedures of this component are:
/// - `distribute`, which mints a unique NFT and creates a note for the provided recipient.
/// - `burn`, which burns the provided non-fungible asset.
/// - `get_token_data`, which reads the token registry by token_id.
/// - `get_owner`, `transfer_ownership`, `renounce_ownership` for owner management.
///
/// Both `distribute` and `burn` can only be called from note scripts. `distribute` requires
/// the note sender to be the owner. `burn` does not require authentication.
///
/// ## Storage Layout
///
/// - [`Self::metadata_slot`]: NFT faucet metadata.
/// - [`Self::owner_config_slot`]: The owner account of this network faucet.
/// - [`Self::token_map_slot`]: Token registry (token_id -> data_hash).
///
/// [builder]: crate::code_builder::CodeBuilder
pub struct NetworkNonFungibleFaucet {
    metadata: NftMetadata,
    owner_account_id: AccountId,
}

impl NetworkNonFungibleFaucet {
    // CONSTANTS
    // --------------------------------------------------------------------------------------------

    /// The name of the component.
    pub const NAME: &'static str = "miden::network_non_fungible_faucet";

    const DISTRIBUTE_PROC_NAME: &str = "network_non_fungible_faucet::distribute";
    const BURN_PROC_NAME: &str = "network_non_fungible_faucet::burn";

    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Creates a new [`NetworkNonFungibleFaucet`] component.
    ///
    /// # Errors
    /// Returns an error if `max_supply` is zero.
    pub fn new(
        symbol: TokenSymbol,
        max_supply: Felt,
        owner_account_id: AccountId,
    ) -> Result<Self, NonFungibleFaucetError> {
        let metadata = NftMetadata::new(symbol, max_supply)?;
        Ok(Self { metadata, owner_account_id })
    }

    /// Creates a new [`NetworkNonFungibleFaucet`] from pre-validated metadata.
    pub fn from_metadata(metadata: NftMetadata, owner_account_id: AccountId) -> Self {
        Self { metadata, owner_account_id }
    }

    /// Attempts to create from account interface and storage.
    fn try_from_interface(
        interface: AccountInterface,
        storage: &AccountStorage,
    ) -> Result<Self, NonFungibleFaucetError> {
        if !interface
            .components()
            .contains(&AccountComponentInterface::NetworkNonFungibleFaucet)
        {
            return Err(NonFungibleFaucetError::MissingNetworkNonFungibleFaucetInterface);
        }

        let metadata = NftMetadata::try_from(storage)?;

        let owner_account_id_word: Word = storage
            .get_item(NetworkNonFungibleFaucet::owner_config_slot())
            .map_err(|err| NonFungibleFaucetError::StorageLookupFailed {
                slot_name: NetworkNonFungibleFaucet::owner_config_slot().clone(),
                source: err,
            })?;

        // Storage format: [0, 0, suffix, prefix]
        let prefix = owner_account_id_word[3];
        let suffix = owner_account_id_word[2];
        let owner_account_id = AccountId::new_unchecked([prefix, suffix]);

        Ok(Self { metadata, owner_account_id })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the [`StorageSlotName`] where the NFT metadata is stored.
    pub fn metadata_slot() -> &'static StorageSlotName {
        NftMetadata::metadata_slot()
    }

    /// Returns the [`StorageSlotName`] where the owner configuration is stored.
    pub fn owner_config_slot() -> &'static StorageSlotName {
        &OWNER_CONFIG_SLOT_NAME
    }

    /// Returns the [`StorageSlotName`] of the token registry map.
    pub fn token_map_slot() -> &'static StorageSlotName {
        &TOKEN_MAP_SLOT_NAME
    }

    /// Returns the storage slot schema for the metadata slot.
    pub fn metadata_slot_schema() -> (StorageSlotName, StorageSlotSchema) {
        let token_symbol_type = SchemaType::new(TOKEN_SYMBOL_TYPE).expect("valid type");
        (
            Self::metadata_slot().clone(),
            StorageSlotSchema::value(
                "NFT faucet metadata",
                [
                    FeltSchema::felt("current_supply").with_default(Felt::new(0)),
                    FeltSchema::felt("max_supply"),
                    FeltSchema::felt("next_token_id").with_default(Felt::new(0)),
                    FeltSchema::new_typed(token_symbol_type, "symbol"),
                ],
            ),
        )
    }

    /// Returns the storage slot schema for the owner configuration slot.
    pub fn owner_config_slot_schema() -> (StorageSlotName, StorageSlotSchema) {
        (
            Self::owner_config_slot().clone(),
            StorageSlotSchema::value(
                "Owner account configuration",
                [
                    FeltSchema::new_void(),
                    FeltSchema::new_void(),
                    FeltSchema::felt("owner_suffix"),
                    FeltSchema::felt("owner_prefix"),
                ],
            ),
        )
    }

    /// Returns the storage slot schema for the token registry map.
    pub fn token_map_slot_schema() -> (StorageSlotName, StorageSlotSchema) {
        (
            Self::token_map_slot().clone(),
            StorageSlotSchema::map(
                "Token registry: token_id -> data_hash",
                SchemaType::u32(),
                SchemaType::native_word(),
            ),
        )
    }

    pub fn metadata(&self) -> &NftMetadata {
        &self.metadata
    }

    pub fn symbol(&self) -> TokenSymbol {
        self.metadata.symbol()
    }

    pub fn max_supply(&self) -> Felt {
        self.metadata.max_supply()
    }

    pub fn current_supply(&self) -> u64 {
        self.metadata.current_supply()
    }

    pub fn next_token_id(&self) -> u64 {
        self.metadata.next_token_id()
    }

    pub fn owner_account_id(&self) -> AccountId {
        self.owner_account_id
    }

    pub fn distribute_digest() -> Word {
        *NETWORK_NON_FUNGIBLE_FAUCET_DISTRIBUTE
    }

    pub fn burn_digest() -> Word {
        *NETWORK_NON_FUNGIBLE_FAUCET_BURN
    }
}

impl From<NetworkNonFungibleFaucet> for AccountComponent {
    fn from(faucet: NetworkNonFungibleFaucet) -> Self {
        let metadata_slot: StorageSlot = faucet.metadata.into();

        let owner_account_id_word: Word = [
            Felt::new(0),
            Felt::new(0),
            faucet.owner_account_id.suffix(),
            faucet.owner_account_id.prefix().as_felt(),
        ]
        .into();

        let owner_slot = StorageSlot::with_value(
            NetworkNonFungibleFaucet::owner_config_slot().clone(),
            owner_account_id_word,
        );

        let token_map_slot = StorageSlot::with_map(
            NetworkNonFungibleFaucet::token_map_slot().clone(),
            StorageMap::default(),
        );

        let storage_schema = StorageSchema::new([
            NetworkNonFungibleFaucet::metadata_slot_schema(),
            NetworkNonFungibleFaucet::owner_config_slot_schema(),
            NetworkNonFungibleFaucet::token_map_slot_schema(),
        ])
        .expect("storage schema should be valid");

        let metadata = AccountComponentMetadata::new(NetworkNonFungibleFaucet::NAME)
            .with_description("Network non-fungible faucet for minting unique NFTs via notes")
            .with_supported_type(AccountType::NonFungibleFaucet)
            .with_storage_schema(storage_schema);

        AccountComponent::new(
            network_non_fungible_faucet_library(),
            vec![metadata_slot, owner_slot, token_map_slot],
            metadata,
        )
        .expect("network non-fungible faucet component should satisfy the requirements of a valid account component")
    }
}

impl TryFrom<Account> for NetworkNonFungibleFaucet {
    type Error = NonFungibleFaucetError;

    fn try_from(account: Account) -> Result<Self, Self::Error> {
        let account_interface = AccountInterface::from_account(&account);

        NetworkNonFungibleFaucet::try_from_interface(account_interface, account.storage())
    }
}

impl TryFrom<&Account> for NetworkNonFungibleFaucet {
    type Error = NonFungibleFaucetError;

    fn try_from(account: &Account) -> Result<Self, Self::Error> {
        let account_interface = AccountInterface::from_account(account);

        NetworkNonFungibleFaucet::try_from_interface(account_interface, account.storage())
    }
}

/// Creates a new faucet account with network non-fungible faucet interface and provided metadata.
///
/// Network non-fungible faucets always use:
/// - [`AccountStorageMode::Network`] for storage
/// - [`NoAuth`] for authentication
///
/// Owner is verified via the ownable component on `distribute` calls.
pub fn create_network_non_fungible_faucet(
    init_seed: [u8; 32],
    symbol: TokenSymbol,
    max_supply: Felt,
    owner_account_id: AccountId,
) -> Result<Account, NonFungibleFaucetError> {
    let auth_component: AccountComponent = NoAuth::new().into();

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::NonFungibleFaucet)
        .storage_mode(AccountStorageMode::Network)
        .with_auth_component(auth_component)
        .with_component(NetworkNonFungibleFaucet::new(
            symbol,
            max_supply,
            owner_account_id,
        )?)
        .build()
        .map_err(NonFungibleFaucetError::AccountError)?;

    Ok(account)
}

