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
use crate::account::AuthMethod;
use crate::account::auth::{AuthSingleSigAcl, AuthSingleSigAclConfig};
use crate::account::components::basic_non_fungible_faucet_library;
use crate::account::interface::{AccountComponentInterface, AccountInterface, AccountInterfaceExt};
use crate::procedure_digest;

// CONSTANTS
// ================================================================================================

/// The schema type for token symbols.
const TOKEN_SYMBOL_TYPE: &str = "miden::standards::fungible_faucets::metadata::token_symbol";

static TOKEN_MAP_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::nft::tokens")
        .expect("storage slot name should be valid")
});

// BASIC NON-FUNGIBLE FAUCET ACCOUNT COMPONENT
// ================================================================================================

procedure_digest!(
    BASIC_NON_FUNGIBLE_FAUCET_DISTRIBUTE,
    BasicNonFungibleFaucet::DISTRIBUTE_PROC_NAME,
    basic_non_fungible_faucet_library
);

procedure_digest!(
    BASIC_NON_FUNGIBLE_FAUCET_BURN,
    BasicNonFungibleFaucet::BURN_PROC_NAME,
    basic_non_fungible_faucet_library
);

/// An [`AccountComponent`] implementing a basic non-fungible faucet.
///
/// It reexports the procedures from `miden::standards::faucets::non_fungible`. When linking
/// against this component, the `miden` library (i.e.
/// [`ProtocolLib`](miden_protocol::ProtocolLib)) must be available to the assembler which is the
/// case when using [`CodeBuilder`][builder]. The procedures of this component are:
/// - `distribute`, which mints a unique NFT and creates a note for the provided recipient.
/// - `burn`, which burns the provided non-fungible asset.
/// - `get_token_data`, which reads the token registry by token_id.
///
/// The `distribute` procedure can be called from a transaction script and requires authentication
/// via the authentication component. The `burn` procedure can only be called from a note script
/// and requires the calling note to contain the asset to be burned.
/// This component must be combined with an authentication component.
///
/// This component supports accounts of type [`AccountType::NonFungibleFaucet`].
///
/// ## Storage Layout
///
/// - [`Self::metadata_slot`]: Stores [`NftMetadata`] as `[current_supply, max_supply, next_token_id, symbol]`.
/// - [`Self::token_map_slot`]: StorageMap of `[token_id, 0, 0, 0] -> data_hash`.
///
/// [builder]: crate::code_builder::CodeBuilder
pub struct BasicNonFungibleFaucet {
    metadata: NftMetadata,
}

impl BasicNonFungibleFaucet {
    // CONSTANTS
    // --------------------------------------------------------------------------------------------

    /// The name of the component.
    pub const NAME: &'static str = "miden::basic_non_fungible_faucet";

    const DISTRIBUTE_PROC_NAME: &str = "basic_non_fungible_faucet::distribute";
    const BURN_PROC_NAME: &str = "basic_non_fungible_faucet::burn";

    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Creates a new [`BasicNonFungibleFaucet`] component with zero supply.
    ///
    /// # Errors
    ///
    /// Returns an error if `max_supply` is zero.
    pub fn new(
        symbol: TokenSymbol,
        max_supply: Felt,
    ) -> Result<Self, NonFungibleFaucetError> {
        let metadata = NftMetadata::new(symbol, max_supply)?;
        Ok(Self { metadata })
    }

    /// Creates a new [`BasicNonFungibleFaucet`] from pre-validated metadata.
    pub fn from_metadata(metadata: NftMetadata) -> Self {
        Self { metadata }
    }

    /// Attempts to create a new [`BasicNonFungibleFaucet`] from the associated account
    /// interface and storage.
    fn try_from_interface(
        interface: AccountInterface,
        storage: &AccountStorage,
    ) -> Result<Self, NonFungibleFaucetError> {
        if !interface
            .components()
            .contains(&AccountComponentInterface::BasicNonFungibleFaucet)
        {
            return Err(NonFungibleFaucetError::MissingBasicNonFungibleFaucetInterface);
        }

        let metadata = NftMetadata::try_from(storage)?;
        Ok(Self { metadata })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the [`StorageSlotName`] where the NFT metadata is stored.
    pub fn metadata_slot() -> &'static StorageSlotName {
        NftMetadata::metadata_slot()
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

    /// Returns the token metadata.
    pub fn metadata(&self) -> &NftMetadata {
        &self.metadata
    }

    /// Returns the symbol of the faucet.
    pub fn symbol(&self) -> TokenSymbol {
        self.metadata.symbol()
    }

    /// Returns the max supply of the faucet.
    pub fn max_supply(&self) -> Felt {
        self.metadata.max_supply()
    }

    /// Returns the current number of active (non-burned) NFTs.
    pub fn current_supply(&self) -> u64 {
        self.metadata.current_supply()
    }

    /// Returns the next token ID that will be assigned.
    pub fn next_token_id(&self) -> u64 {
        self.metadata.next_token_id()
    }

    /// Returns the digest of the `distribute` account procedure.
    pub fn distribute_digest() -> Word {
        *BASIC_NON_FUNGIBLE_FAUCET_DISTRIBUTE
    }

    /// Returns the digest of the `burn` account procedure.
    pub fn burn_digest() -> Word {
        *BASIC_NON_FUNGIBLE_FAUCET_BURN
    }
}

impl From<BasicNonFungibleFaucet> for AccountComponent {
    fn from(faucet: BasicNonFungibleFaucet) -> Self {
        let metadata_slot: StorageSlot = faucet.metadata.into();

        let token_map_slot = StorageSlot::with_map(
            BasicNonFungibleFaucet::token_map_slot().clone(),
            StorageMap::default(),
        );

        let storage_schema = StorageSchema::new([
            BasicNonFungibleFaucet::metadata_slot_schema(),
            BasicNonFungibleFaucet::token_map_slot_schema(),
        ])
        .expect("storage schema should be valid");

        let metadata = AccountComponentMetadata::new(BasicNonFungibleFaucet::NAME)
            .with_description("Non-fungible faucet for minting unique NFTs")
            .with_supported_type(AccountType::NonFungibleFaucet)
            .with_storage_schema(storage_schema);

        AccountComponent::new(
            basic_non_fungible_faucet_library(),
            vec![metadata_slot, token_map_slot],
            metadata,
        )
        .expect("basic non-fungible faucet component should satisfy the requirements of a valid account component")
    }
}

impl TryFrom<Account> for BasicNonFungibleFaucet {
    type Error = NonFungibleFaucetError;

    fn try_from(account: Account) -> Result<Self, Self::Error> {
        let account_interface = AccountInterface::from_account(&account);

        BasicNonFungibleFaucet::try_from_interface(account_interface, account.storage())
    }
}

impl TryFrom<&Account> for BasicNonFungibleFaucet {
    type Error = NonFungibleFaucetError;

    fn try_from(account: &Account) -> Result<Self, Self::Error> {
        let account_interface = AccountInterface::from_account(account);

        BasicNonFungibleFaucet::try_from_interface(account_interface, account.storage())
    }
}

/// Creates a new faucet account with basic non-fungible faucet interface.
///
/// The basic non-fungible faucet interface exposes three procedures:
/// - `distribute`, which mints a unique NFT and creates a note for the provided recipient.
/// - `burn`, which burns the provided non-fungible asset.
/// - `get_token_data`, which reads the token registry by token_id.
///
/// The `distribute` procedure can be called from a transaction script and requires authentication
/// via the specified authentication method. The `burn` procedure can only be called from a note
/// script and requires the calling note to contain the asset to be burned.
pub fn create_basic_non_fungible_faucet(
    init_seed: [u8; 32],
    symbol: TokenSymbol,
    max_supply: Felt,
    account_storage_mode: AccountStorageMode,
    auth_method: AuthMethod,
) -> Result<Account, NonFungibleFaucetError> {
    let distribute_proc_root = BasicNonFungibleFaucet::distribute_digest();

    let auth_component: AccountComponent = match auth_method {
        AuthMethod::SingleSig { approver: (pub_key, auth_scheme) } => AuthSingleSigAcl::new(
            pub_key,
            auth_scheme,
            AuthSingleSigAclConfig::new()
                .with_auth_trigger_procedures(vec![distribute_proc_root])
                .with_allow_unauthorized_input_notes(true),
        )
        .map_err(NonFungibleFaucetError::AccountError)?
        .into(),
        AuthMethod::NoAuth => {
            return Err(NonFungibleFaucetError::UnsupportedAuthMethod(
                "basic non-fungible faucets cannot be created with NoAuth authentication method"
                    .into(),
            ));
        },
        AuthMethod::Unknown => {
            return Err(NonFungibleFaucetError::UnsupportedAuthMethod(
                "basic non-fungible faucets cannot be created with Unknown authentication method"
                    .into(),
            ));
        },
        AuthMethod::Multisig { .. } => {
            return Err(NonFungibleFaucetError::UnsupportedAuthMethod(
                "basic non-fungible faucets do not support Multisig authentication".into(),
            ));
        },
    };

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::NonFungibleFaucet)
        .storage_mode(account_storage_mode)
        .with_auth_component(auth_component)
        .with_component(BasicNonFungibleFaucet::new(symbol, max_supply)?)
        .build()
        .map_err(NonFungibleFaucetError::AccountError)?;

    Ok(account)
}

