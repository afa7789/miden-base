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
use miden_protocol::asset::{FungibleAsset, TokenSymbol};
use miden_protocol::{Felt, FieldElement, Word};

use super::token_logo_uri::TokenLogoURI;
use super::token_name::TokenName;
use super::{
    FungibleFaucetError, TOKEN_LOGO_URI_DOUBLE_WORD_INDEX_START, TOKEN_NAME_DOUBLE_WORD_INDEX,
    double_word_map_keys, metadata_map_key_word0, metadata_map_key_word1,
};
use crate::account::AuthScheme;
use crate::account::auth::{
    AuthEcdsaK256KeccakAcl,
    AuthEcdsaK256KeccakAclConfig,
    AuthFalcon512RpoAcl,
    AuthFalcon512RpoAclConfig,
};
use crate::account::components::basic_fungible_faucet_library;
use crate::account::interface::{AccountComponentInterface, AccountInterface, AccountInterfaceExt};
use crate::procedure_digest;

// BASIC FUNGIBLE FAUCET ACCOUNT COMPONENT
// ================================================================================================

// Initialize the digest of the `distribute` procedure of the Basic Fungible Faucet only once.
procedure_digest!(
    BASIC_FUNGIBLE_FAUCET_DISTRIBUTE,
    BasicFungibleFaucet::DISTRIBUTE_PROC_NAME,
    basic_fungible_faucet_library
);

// Initialize the digest of the `burn` procedure of the Basic Fungible Faucet only once.
procedure_digest!(
    BASIC_FUNGIBLE_FAUCET_BURN,
    BasicFungibleFaucet::BURN_PROC_NAME,
    basic_fungible_faucet_library
);

/// An [`AccountComponent`] implementing a basic fungible faucet.
///
/// It reexports the procedures from `miden::standards::faucets::basic_fungible`. When linking
/// against this component, the `miden` library (i.e.
/// [`ProtocolLib`](miden_protocol::ProtocolLib)) must be available to the assembler which is the
/// case when using [`CodeBuilder`][builder]. The procedures of this component are:
/// - `distribute`, which mints an assets and create a note for the provided recipient.
/// - `burn`, which burns the provided asset.
///
/// The `distribute` procedure can be called from a transaction script and requires authentication
/// via the authentication component. The `burn` procedure can only be called from a note script
/// and requires the calling note to contain the asset to be burned.
/// This component must be combined with an authentication component.
///
/// This component supports accounts of type [`AccountType::FungibleFaucet`].
///
/// ## Storage Layout
///
/// All data is stored in a single map slot ([`Self::metadata_slot`]) using the double_word_array
/// layout:
///
/// - **Index 0** (core metadata):
///   - **Word0** `[token_supply, max_supply, decimals, token_symbol]`
///   - **Word1** `[reserved, reserved, reserved, reserved]`
///
/// - **Index 1** (token name):
///   - **Word0 / Word1**: [`TokenName`] encoded as 2 words (up to 32 bytes UTF-8).
///
/// - **Indices 2–5** (token logo URI):
///   - 4 double-words (8 words): [`TokenLogoURI`] encoded as 8 words (up to 128 bytes UTF-8).
///
/// [builder]: crate::code_builder::CodeBuilder
#[derive(Debug)]
pub struct BasicFungibleFaucet {
    token_supply: Felt,
    max_supply: Felt,
    decimals: u8,
    symbol: TokenSymbol,
    name: TokenName,
    logo_uri: Option<TokenLogoURI>,
}

impl BasicFungibleFaucet {
    // CONSTANTS
    // --------------------------------------------------------------------------------------------

    /// The maximum number of decimals supported by the component.
    pub const MAX_DECIMALS: u8 = 12;

    const DISTRIBUTE_PROC_NAME: &str = "basic_fungible_faucet::distribute";
    const BURN_PROC_NAME: &str = "basic_fungible_faucet::burn";

    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Creates a new [`BasicFungibleFaucet`] component from the given pieces of metadata and with
    /// an initial token supply of zero.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the decimals parameter exceeds maximum value of [`Self::MAX_DECIMALS`].
    /// - the max supply parameter exceeds maximum possible amount for a fungible asset
    ///   ([`FungibleAsset::MAX_AMOUNT`])
    pub fn new(
        symbol: TokenSymbol,
        decimals: u8,
        max_supply: Felt,
        name: TokenName,
    ) -> Result<Self, FungibleFaucetError> {
        // First check that the metadata is valid.
        if decimals > Self::MAX_DECIMALS {
            return Err(FungibleFaucetError::TooManyDecimals {
                actual: decimals as u64,
                max: Self::MAX_DECIMALS,
            });
        } else if max_supply.as_int() > FungibleAsset::MAX_AMOUNT {
            return Err(FungibleFaucetError::MaxSupplyTooLarge {
                actual: max_supply.as_int(),
                max: FungibleAsset::MAX_AMOUNT,
            });
        }

        Ok(Self {
            token_supply: Felt::ZERO,
            max_supply,
            decimals,
            symbol,
            name,
            logo_uri: None,
        })
    }

    /// Attempts to create a new [`BasicFungibleFaucet`] component from the associated account
    /// interface and storage.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the provided [`AccountInterface`] does not contain a
    ///   [`AccountComponentInterface::BasicFungibleFaucet`] component.
    /// - the decimals parameter exceeds maximum value of [`Self::MAX_DECIMALS`].
    /// - the max supply value exceeds maximum possible amount for a fungible asset of
    ///   [`FungibleAsset::MAX_AMOUNT`].
    /// - the token supply exceeds the max supply.
    /// - the token symbol encoded value exceeds the maximum value of
    ///   [`TokenSymbol::MAX_ENCODED_VALUE`].
    fn try_from_interface(
        interface: AccountInterface,
        storage: &AccountStorage,
    ) -> Result<Self, FungibleFaucetError> {
        // Check that the procedures of the basic fungible faucet exist in the account.
        if !interface.components().contains(&AccountComponentInterface::BasicFungibleFaucet) {
            return Err(FungibleFaucetError::MissingBasicFungibleFaucetInterface);
        }

        Self::try_from_storage(storage)
    }

    /// Attempts to create a new [`BasicFungibleFaucet`] from the provided account storage.
    ///
    /// # Warning
    ///
    /// This does not check for the presence of the faucet's procedures.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the decimals parameter exceeds maximum value of [`Self::MAX_DECIMALS`].
    /// - the max supply value exceeds maximum possible amount for a fungible asset of
    ///   [`FungibleAsset::MAX_AMOUNT`].
    /// - the token supply exceeds the max supply.
    /// - the token symbol encoded value exceeds the maximum value of
    ///   [`TokenSymbol::MAX_ENCODED_VALUE`].
    pub(super) fn try_from_storage(storage: &AccountStorage) -> Result<Self, FungibleFaucetError> {
        let metadata_slot_name = BasicFungibleFaucet::metadata_slot();

        // Read core metadata (index 0).
        let word0 = storage
            .get_map_item(metadata_slot_name, metadata_map_key_word0())
            .map_err(|err| FungibleFaucetError::StorageLookupFailed {
                slot_name: metadata_slot_name.clone(),
                source: err,
            })?;

        let [token_supply, max_supply, decimals, token_symbol] = *word0;

        // Convert token symbol and decimals to expected types.
        let token_symbol =
            TokenSymbol::try_from(token_symbol).map_err(FungibleFaucetError::InvalidTokenSymbol)?;
        let decimals =
            decimals.as_int().try_into().map_err(|_| FungibleFaucetError::TooManyDecimals {
                actual: decimals.as_int(),
                max: Self::MAX_DECIMALS,
            })?;

        // Read token name (index 1).
        let (name_key0, name_key1) = double_word_map_keys(TOKEN_NAME_DOUBLE_WORD_INDEX);
        let name_word0 = storage
            .get_map_item(metadata_slot_name, name_key0)
            .map_err(|err| FungibleFaucetError::StorageLookupFailed {
                slot_name: metadata_slot_name.clone(),
                source: err,
            })?;
        let name_word1 = storage
            .get_map_item(metadata_slot_name, name_key1)
            .map_err(|err| FungibleFaucetError::StorageLookupFailed {
                slot_name: metadata_slot_name.clone(),
                source: err,
            })?;
        let name = TokenName::try_from([name_word0, name_word1])
            .map_err(FungibleFaucetError::InvalidTokenName)?;

        // Read token logo URI (indices 2–5, 4 double-words = 8 words).
        let logo_uri = Self::read_logo_uri_from_storage(storage, metadata_slot_name)?;

        let mut faucet = BasicFungibleFaucet::new(token_symbol, decimals, max_supply, name)?;
        faucet.logo_uri = logo_uri;
        faucet.with_token_supply(token_supply)
    }

    /// Reads a [`TokenLogoURI`] from storage at indices 2–5. Returns `None` if all words are
    /// zero (no URI stored).
    fn read_logo_uri_from_storage(
        storage: &AccountStorage,
        slot_name: &StorageSlotName,
    ) -> Result<Option<TokenLogoURI>, FungibleFaucetError> {
        let mut uri_words = [Word::default(); 8];
        let mut all_zero = true;

        for i in 0..4u64 {
            let index = TOKEN_LOGO_URI_DOUBLE_WORD_INDEX_START + i;
            let (key0, key1) = double_word_map_keys(index);

            let w0 = storage.get_map_item(slot_name, key0).map_err(|err| {
                FungibleFaucetError::StorageLookupFailed {
                    slot_name: slot_name.clone(),
                    source: err,
                }
            })?;
            let w1 = storage.get_map_item(slot_name, key1).map_err(|err| {
                FungibleFaucetError::StorageLookupFailed {
                    slot_name: slot_name.clone(),
                    source: err,
                }
            })?;

            uri_words[i as usize * 2] = w0;
            uri_words[i as usize * 2 + 1] = w1;

            if w0 != Word::default() || w1 != Word::default() {
                all_zero = false;
            }
        }

        if all_zero {
            return Ok(None);
        }

        let uri = TokenLogoURI::try_from(uri_words)
            .map_err(FungibleFaucetError::InvalidTokenLogoURI)?;
        Ok(Some(uri))
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the [`StorageSlotName`] where the [`BasicFungibleFaucet`]'s metadata is stored.
    pub fn metadata_slot() -> &'static StorageSlotName {
        &super::METADATA_SLOT_NAME
    }

    /// Returns the symbol of the faucet.
    pub fn symbol(&self) -> TokenSymbol {
        self.symbol
    }

    /// Returns the decimals of the faucet.
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Returns the max supply (in base units) of the faucet.
    ///
    /// This is the highest amount of tokens that can be minted from this faucet.
    pub fn max_supply(&self) -> Felt {
        self.max_supply
    }

    /// Returns the token supply (in base units) of the faucet.
    ///
    /// This is the amount of tokens that were minted from the faucet so far. Its value can never
    /// exceed [`Self::max_supply`].
    pub fn token_supply(&self) -> Felt {
        self.token_supply
    }

    /// Returns the token name.
    pub fn name(&self) -> &TokenName {
        &self.name
    }

    /// Returns the token logo URI, if set.
    pub fn logo_uri(&self) -> Option<&TokenLogoURI> {
        self.logo_uri.as_ref()
    }

    /// Returns the digest of the `distribute` account procedure.
    pub fn distribute_digest() -> Word {
        *BASIC_FUNGIBLE_FAUCET_DISTRIBUTE
    }

    /// Returns the digest of the `burn` account procedure.
    pub fn burn_digest() -> Word {
        *BASIC_FUNGIBLE_FAUCET_BURN
    }

    /// Returns all storage map entries for this faucet's metadata.
    ///
    /// This produces entries for:
    /// - Index 0: core metadata (symbol, decimals, supply)
    /// - Index 1: token name
    /// - Indices 2–5: token logo URI (if set)
    pub(super) fn to_storage_map_entries(&self) -> alloc::vec::Vec<(Word, Word)> {
        let mut entries = alloc::vec::Vec::new();

        // Index 0: core metadata.
        let word0 = Word::new([
            self.token_supply,
            self.max_supply,
            Felt::from(self.decimals),
            Felt::from(self.symbol),
        ]);
        let word1 = Word::default(); // reserved
        entries.push((metadata_map_key_word0(), word0));
        entries.push((metadata_map_key_word1(), word1));

        // Index 1: token name.
        let [name_w0, name_w1] = *self.name.words();
        let (name_key0, name_key1) = double_word_map_keys(TOKEN_NAME_DOUBLE_WORD_INDEX);
        entries.push((name_key0, name_w0));
        entries.push((name_key1, name_w1));

        // Indices 2–5: token logo URI (if set).
        if let Some(ref uri) = self.logo_uri {
            let uri_words = uri.words();
            for i in 0..4u64 {
                let index = TOKEN_LOGO_URI_DOUBLE_WORD_INDEX_START + i;
                let (key0, key1) = double_word_map_keys(index);
                entries.push((key0, uri_words[i as usize * 2]));
                entries.push((key1, uri_words[i as usize * 2 + 1]));
            }
        }

        entries
    }

    // MUTATORS
    // --------------------------------------------------------------------------------------------

    /// Sets the token_supply (in base units) of the basic fungible faucet.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the token supply exceeds the max supply.
    pub fn with_token_supply(mut self, token_supply: Felt) -> Result<Self, FungibleFaucetError> {
        if token_supply.as_int() > self.max_supply.as_int() {
            return Err(FungibleFaucetError::TokenSupplyExceedsMaxSupply {
                token_supply: token_supply.as_int(),
                max_supply: self.max_supply.as_int(),
            });
        }

        self.token_supply = token_supply;

        Ok(self)
    }

    /// Sets the token logo URI.
    pub fn with_logo_uri(mut self, logo_uri: TokenLogoURI) -> Self {
        self.logo_uri = Some(logo_uri);
        self
    }
}

impl From<BasicFungibleFaucet> for AccountComponent {
    fn from(faucet: BasicFungibleFaucet) -> Self {
        let entries = faucet.to_storage_map_entries();
        let metadata_map =
            StorageMap::with_entries(entries).expect("metadata map keys are distinct");
        let storage_slot = StorageSlot::with_map(
            BasicFungibleFaucet::metadata_slot().clone(),
            metadata_map,
        );

        AccountComponent::new(basic_fungible_faucet_library(), vec![storage_slot])
            .expect("basic fungible faucet component should satisfy the requirements of a valid account component")
            .with_supported_type(AccountType::FungibleFaucet)
    }
}

impl TryFrom<Account> for BasicFungibleFaucet {
    type Error = FungibleFaucetError;

    fn try_from(account: Account) -> Result<Self, Self::Error> {
        let account_interface = AccountInterface::from_account(&account);

        BasicFungibleFaucet::try_from_interface(account_interface, account.storage())
    }
}

impl TryFrom<&Account> for BasicFungibleFaucet {
    type Error = FungibleFaucetError;

    fn try_from(account: &Account) -> Result<Self, Self::Error> {
        let account_interface = AccountInterface::from_account(account);

        BasicFungibleFaucet::try_from_interface(account_interface, account.storage())
    }
}

/// Creates a new faucet account with basic fungible faucet interface,
/// account storage type, specified authentication scheme, and provided meta data (token symbol,
/// decimals, max supply).
///
/// The basic faucet interface exposes two procedures:
/// - `distribute`, which mints an assets and create a note for the provided recipient.
/// - `burn`, which burns the provided asset.
///
/// The `distribute` procedure can be called from a transaction script and requires authentication
/// via the specified authentication scheme. The `burn` procedure can only be called from a note
/// script and requires the calling note to contain the asset to be burned.
///
/// The storage layout of the faucet account is defined by the combination of the following
/// components (see their docs for details):
/// - [`BasicFungibleFaucet`]
/// - [`AuthEcdsaK256KeccakAcl`] or [`AuthFalcon512RpoAcl`]
pub fn create_basic_fungible_faucet(
    init_seed: [u8; 32],
    symbol: TokenSymbol,
    decimals: u8,
    max_supply: Felt,
    name: TokenName,
    account_storage_mode: AccountStorageMode,
    auth_scheme: AuthScheme,
) -> Result<Account, FungibleFaucetError> {
    let distribute_proc_root = BasicFungibleFaucet::distribute_digest();

    let auth_component: AccountComponent = match auth_scheme {
        AuthScheme::Falcon512Rpo { pub_key } => AuthFalcon512RpoAcl::new(
            pub_key,
            AuthFalcon512RpoAclConfig::new()
                .with_auth_trigger_procedures(vec![distribute_proc_root])
                .with_allow_unauthorized_input_notes(true),
        )
        .map_err(FungibleFaucetError::AccountError)?
        .into(),
        AuthScheme::EcdsaK256Keccak { pub_key } => AuthEcdsaK256KeccakAcl::new(
            pub_key,
            AuthEcdsaK256KeccakAclConfig::new()
                .with_auth_trigger_procedures(vec![distribute_proc_root])
                .with_allow_unauthorized_input_notes(true),
        )
        .map_err(FungibleFaucetError::AccountError)?
        .into(),
        AuthScheme::NoAuth => {
            return Err(FungibleFaucetError::UnsupportedAuthScheme(
                "basic fungible faucets cannot be created with NoAuth authentication scheme".into(),
            ));
        },
        AuthScheme::Falcon512RpoMultisig { threshold: _, pub_keys: _ } => {
            return Err(FungibleFaucetError::UnsupportedAuthScheme(
                "basic fungible faucets do not support multisig authentication".into(),
            ));
        },
        AuthScheme::Unknown => {
            return Err(FungibleFaucetError::UnsupportedAuthScheme(
                "basic fungible faucets cannot be created with Unknown authentication scheme"
                    .into(),
            ));
        },
        AuthScheme::EcdsaK256KeccakMultisig { threshold: _, pub_keys: _ } => {
            return Err(FungibleFaucetError::UnsupportedAuthScheme(
                "basic fungible faucets do not support EcdsaK256KeccakMultisig authentication"
                    .into(),
            ));
        },
    };

    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(account_storage_mode)
        .with_auth_component(auth_component)
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply, name)?)
        .build()
        .map_err(FungibleFaucetError::AccountError)?;

    Ok(account)
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use assert_matches::assert_matches;
    use miden_protocol::account::auth::PublicKeyCommitment;
    use miden_protocol::asset::FungibleAsset;
    use miden_protocol::{FieldElement, ONE, Word};

    use super::{
        AccountBuilder,
        AccountComponent,
        AccountStorageMode,
        AccountType,
        BasicFungibleFaucet,
        Felt,
        FungibleFaucetError,
        TokenLogoURI,
        TokenName,
        TokenSymbol,
        double_word_map_keys,
        metadata_map_key_word0,
    };
    use crate::account::auth::{
        AuthFalcon512Rpo,
        AuthFalcon512RpoAcl,
        AuthFalcon512RpoAclConfig,
    };
    use crate::account::faucets::TOKEN_NAME_DOUBLE_WORD_INDEX;
    use crate::account::wallets::BasicWallet;

    #[test]
    fn faucet_contract_creation() {
        let pub_key_word = Word::new([ONE; 4]);

        // we need to use an initial seed to create the wallet account
        let init_seed: [u8; 32] = [
            90, 110, 209, 94, 84, 105, 250, 242, 223, 203, 216, 124, 22, 159, 14, 132, 215, 85,
            183, 204, 149, 90, 166, 68, 100, 73, 106, 168, 125, 237, 138, 16,
        ];

        let max_supply = Felt::new(123);
        let token_symbol_string = "POL";
        let token_symbol = TokenSymbol::try_from(token_symbol_string).unwrap();
        let decimals = 2u8;
        let name = TokenName::new("Polygon").unwrap();
        let logo_uri = TokenLogoURI::new("https://example.com/pol.png").unwrap();
        let storage_mode = AccountStorageMode::Private;

        let distribute_proc_root = BasicFungibleFaucet::distribute_digest();
        let auth_component: AccountComponent = AuthFalcon512RpoAcl::new(
            pub_key_word.into(),
            AuthFalcon512RpoAclConfig::new()
                .with_auth_trigger_procedures(vec![distribute_proc_root])
                .with_allow_unauthorized_input_notes(true),
        )
        .map_err(FungibleFaucetError::AccountError)
        .unwrap()
        .into();
        let faucet_component = BasicFungibleFaucet::new(
            token_symbol,
            decimals,
            max_supply,
            name,
        )
        .unwrap()
        .with_logo_uri(logo_uri);
        let faucet_account = AccountBuilder::new(init_seed)
            .account_type(AccountType::FungibleFaucet)
            .storage_mode(storage_mode)
            .with_auth_component(auth_component)
            .with_component(faucet_component)
            .build()
            .map_err(FungibleFaucetError::AccountError)
            .unwrap();

        // The falcon auth component's public key should be present.
        assert_eq!(
            faucet_account
                .storage()
                .get_item(AuthFalcon512RpoAcl::public_key_slot())
                .unwrap(),
            pub_key_word
        );

        // The config slot of the auth component stores:
        // [num_trigger_procs, allow_unauthorized_output_notes, allow_unauthorized_input_notes, 0].
        //
        // With 1 trigger procedure (distribute), allow_unauthorized_output_notes=false, and
        // allow_unauthorized_input_notes=true, this should be [1, 0, 1, 0].
        assert_eq!(
            faucet_account.storage().get_item(AuthFalcon512RpoAcl::config_slot()).unwrap(),
            [Felt::ONE, Felt::ZERO, Felt::ONE, Felt::ZERO].into()
        );

        // The procedure root map should contain the distribute procedure root.
        let distribute_root = BasicFungibleFaucet::distribute_digest();
        assert_eq!(
            faucet_account
                .storage()
                .get_map_item(
                    AuthFalcon512RpoAcl::trigger_procedure_roots_slot(),
                    [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::ZERO].into()
                )
                .unwrap(),
            distribute_root
        );

        // Check that faucet core metadata (index 0 word0) was initialized.
        let word0 = faucet_account
            .storage()
            .get_map_item(BasicFungibleFaucet::metadata_slot(), metadata_map_key_word0())
            .unwrap();
        assert_eq!(
            word0,
            [Felt::ZERO, Felt::new(123), Felt::new(2), token_symbol.into()].into()
        );

        // Check that token name (index 1) was stored.
        let (name_key0, name_key1) = double_word_map_keys(TOKEN_NAME_DOUBLE_WORD_INDEX);
        let stored_name_w0 = faucet_account
            .storage()
            .get_map_item(BasicFungibleFaucet::metadata_slot(), name_key0)
            .unwrap();
        let stored_name_w1 = faucet_account
            .storage()
            .get_map_item(BasicFungibleFaucet::metadata_slot(), name_key1)
            .unwrap();
        let stored_name = TokenName::try_from([stored_name_w0, stored_name_w1]).unwrap();
        assert_eq!(stored_name.to_string(), "Polygon");

        assert!(faucet_account.is_faucet());
        assert_eq!(faucet_account.account_type(), AccountType::FungibleFaucet);

        // Verify the faucet can be extracted and has correct metadata
        let faucet_component = BasicFungibleFaucet::try_from(faucet_account.clone()).unwrap();
        assert_eq!(faucet_component.symbol(), token_symbol);
        assert_eq!(faucet_component.decimals(), decimals);
        assert_eq!(faucet_component.max_supply(), max_supply);
        assert_eq!(faucet_component.token_supply(), Felt::ZERO);
        assert_eq!(faucet_component.name().to_string(), "Polygon");
        assert_eq!(
            faucet_component.logo_uri().unwrap().to_string(),
            "https://example.com/pol.png"
        );
    }

    #[test]
    fn faucet_create_from_account() {
        // prepare the test data
        let mock_word = Word::from([0, 1, 2, 3u32]);
        let mock_public_key = PublicKeyCommitment::from(mock_word);
        let mock_seed = mock_word.as_bytes();

        let name = TokenName::new("Polygon").unwrap();

        // valid account
        let token_symbol = TokenSymbol::new("POL").expect("invalid token symbol");
        let faucet_account = AccountBuilder::new(mock_seed)
            .account_type(AccountType::FungibleFaucet)
            .with_component(
                BasicFungibleFaucet::new(token_symbol, 10, Felt::new(100), name)
                    .expect("failed to create a fungible faucet component"),
            )
            .with_auth_component(AuthFalcon512Rpo::new(mock_public_key))
            .build_existing()
            .expect("failed to create wallet account");

        let basic_ff = BasicFungibleFaucet::try_from(faucet_account)
            .expect("basic fungible faucet creation failed");
        assert_eq!(basic_ff.symbol(), token_symbol);
        assert_eq!(basic_ff.decimals(), 10);
        assert_eq!(basic_ff.max_supply(), Felt::new(100));
        assert_eq!(basic_ff.token_supply(), Felt::ZERO);
        assert_eq!(basic_ff.name().to_string(), "Polygon");
        assert!(basic_ff.logo_uri().is_none());

        // valid account built with with_logo_uri and with_token_supply
        let name2 = TokenName::new("My Token").unwrap();
        let logo_uri = TokenLogoURI::new("https://example.com/logo.png").unwrap();
        let token_supply = Felt::new(50);
        let faucet_with_uri = BasicFungibleFaucet::new(
            token_symbol,
            10,
            Felt::new(100),
            name2,
        )
        .expect("new should succeed")
        .with_logo_uri(logo_uri)
        .with_token_supply(token_supply)
        .expect("with_token_supply should succeed");
        let faucet_account_2 = AccountBuilder::new(mock_seed)
            .account_type(AccountType::FungibleFaucet)
            .with_component(faucet_with_uri)
            .with_auth_component(AuthFalcon512Rpo::new(mock_public_key))
            .build_existing()
            .expect("failed to build account");
        let basic_ff_2 = BasicFungibleFaucet::try_from(faucet_account_2)
            .expect("basic fungible faucet creation failed");
        assert_eq!(basic_ff_2.symbol(), token_symbol);
        assert_eq!(basic_ff_2.decimals(), 10);
        assert_eq!(basic_ff_2.max_supply(), Felt::new(100));
        assert_eq!(basic_ff_2.token_supply(), token_supply);
        assert_eq!(basic_ff_2.name().to_string(), "My Token");
        assert_eq!(
            basic_ff_2.logo_uri().unwrap().to_string(),
            "https://example.com/logo.png"
        );

        // invalid account: basic fungible faucet component is missing
        let invalid_faucet_account = AccountBuilder::new(mock_seed)
            .account_type(AccountType::FungibleFaucet)
            .with_auth_component(AuthFalcon512Rpo::new(mock_public_key))
            // we need to add some other component so the builder doesn't fail
            .with_component(BasicWallet)
            .build_existing()
            .expect("failed to create wallet account");

        let err = BasicFungibleFaucet::try_from(invalid_faucet_account)
            .err()
            .expect("basic fungible faucet creation should fail");
        assert_matches!(err, FungibleFaucetError::MissingBasicFungibleFaucetInterface);
    }

    #[test]
    fn new_rejects_too_many_decimals() {
        let symbol = TokenSymbol::new("POL").expect("invalid token symbol");
        let max_supply = Felt::new(100);
        let name = TokenName::new("Polygon").unwrap();
        let decimals = BasicFungibleFaucet::MAX_DECIMALS + 1;

        let err = BasicFungibleFaucet::new(symbol, decimals, max_supply, name)
            .expect_err("should fail");
        assert_matches!(err, FungibleFaucetError::TooManyDecimals { actual, max } if actual == decimals as u64 && max == BasicFungibleFaucet::MAX_DECIMALS);
    }

    #[test]
    fn new_rejects_max_supply_too_large() {
        let symbol = TokenSymbol::new("POL").expect("invalid token symbol");
        let decimals = 2u8;
        let max_supply = Felt::new(FungibleAsset::MAX_AMOUNT + 1);
        let name = TokenName::new("Polygon").unwrap();

        let err = BasicFungibleFaucet::new(symbol, decimals, max_supply, name)
            .expect_err("should fail");
        assert_matches!(err, FungibleFaucetError::MaxSupplyTooLarge { actual, max } if actual == FungibleAsset::MAX_AMOUNT + 1 && max == FungibleAsset::MAX_AMOUNT);
    }

    #[test]
    fn with_token_supply_succeeds_within_range() {
        let symbol = TokenSymbol::new("POL").expect("invalid token symbol");
        let max_supply = Felt::new(100);
        let name = TokenName::new("Polygon").unwrap();

        let faucet_zero = BasicFungibleFaucet::new(symbol, 2u8, max_supply, name)
            .expect("new should succeed")
            .with_token_supply(Felt::ZERO)
            .expect("with_token_supply(0) should succeed");
        assert_eq!(faucet_zero.token_supply(), Felt::ZERO);

        let symbol = TokenSymbol::new("POL").expect("invalid token symbol");
        let name = TokenName::new("Polygon").unwrap();
        let faucet_max = BasicFungibleFaucet::new(symbol, 2u8, max_supply, name)
            .expect("new should succeed")
            .with_token_supply(max_supply)
            .expect("with_token_supply(max_supply) should succeed");
        assert_eq!(faucet_max.token_supply(), max_supply);
    }

    #[test]
    fn with_token_supply_rejects_exceeds_max_supply() {
        let symbol = TokenSymbol::new("POL").expect("invalid token symbol");
        let max_supply = Felt::new(100);
        let name = TokenName::new("Polygon").unwrap();
        let faucet = BasicFungibleFaucet::new(symbol, 2u8, max_supply, name).expect("new should succeed");
        let token_supply = Felt::new(101);

        let err = faucet.with_token_supply(token_supply).expect_err("with_token_supply(101) should fail");
        assert_matches!(err, FungibleFaucetError::TokenSupplyExceedsMaxSupply { token_supply: ts, max_supply: ms } if ts == 101 && ms == 100);
    }

    /// Check that the obtaining of the basic fungible faucet procedure digests does not panic.
    #[test]
    fn get_faucet_procedures() {
        let _distribute_digest = BasicFungibleFaucet::distribute_digest();
        let _burn_digest = BasicFungibleFaucet::burn_digest();
    }

    #[test]
    fn faucet_without_logo_uri_roundtrips() {
        let mock_word = Word::from([0, 1, 2, 3u32]);
        let mock_public_key = PublicKeyCommitment::from(mock_word);
        let mock_seed = mock_word.as_bytes();

        let token_symbol = TokenSymbol::new("ETH").expect("invalid token symbol");
        let name = TokenName::new("Ether").unwrap();

        let faucet = BasicFungibleFaucet::new(token_symbol, 8, Felt::new(1000), name)
            .expect("new should succeed");
        assert!(faucet.logo_uri().is_none());

        let account = AccountBuilder::new(mock_seed)
            .account_type(AccountType::FungibleFaucet)
            .with_component(faucet)
            .with_auth_component(AuthFalcon512Rpo::new(mock_public_key))
            .build_existing()
            .expect("failed to build account");

        let extracted = BasicFungibleFaucet::try_from(account).unwrap();
        assert_eq!(extracted.name().to_string(), "Ether");
        assert!(extracted.logo_uri().is_none());
    }
}
