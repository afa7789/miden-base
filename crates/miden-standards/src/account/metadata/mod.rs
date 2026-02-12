//! Account / contract / faucet metadata (slots 0..9)
//!
//! All of the following are metadata of the account (or faucet): token_symbol, decimals,
//! max_supply, owner, name, and content URI.
//!
//! ## Storage layout
//!
//! | Slot name | Contents |
//! |-----------|----------|
//! | `metadata::token_metadata` | `[max_supply, decimals, token_symbol, 0]` |
//! | `ownable::owner_config` | owner account id (defined by ownable module) |
//! | `metadata::name_0` | first 4 felts of name |
//! | `metadata::name_1` | last 4 felts of name |
//! | `metadata::content_uri_0..5` | content URI (6 Words, 24 felts) |
//!
//! Slot names use the `miden::standards::metadata::*` namespace, except for the
//! owner which is defined by the ownable module (`miden::standards::access::ownable::owner_config`).
//!
//! Layout sync: the same layout is defined in MASM at `asm/standards/metadata/mod.masm`.
//! Any change to slot indices or names must be applied in both Rust and MASM.
//!
//! # Example
//!
//! ```ignore
//! use miden_standards::account::metadata::Info;
//!
//! let info = Info::new()
//!     .with_name([name_word_0, name_word_1])
//!     .with_content_uri([uri_0, uri_1, uri_2, uri_3, uri_4, uri_5]);
//!
//! let account = AccountBuilder::new(seed)
//!     .with_component(info)
//!     .build()?;
//! ```

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use miden_protocol::Word;
use miden_protocol::account::component::{AccountComponentMetadata, StorageSchema};
use miden_protocol::account::{
    AccountComponent, AccountStorage, StorageSlot, StorageSlotName,
};
use miden_protocol::errors::ComponentMetadataError;
use miden_protocol::utils::sync::LazyLock;

use crate::account::components::{metadata_info_component_library, storage_schema_library};

// CONSTANTS — canonical layout: slots 0–9
// ================================================================================================

/// Token metadata: `[max_supply, decimals, token_symbol, 0]`.
pub static TOKEN_METADATA_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::token_metadata")
        .expect("storage slot name should be valid")
});

/// Owner config — defined by the ownable module (`miden::standards::access::ownable`).
/// Referenced here so that faucets and other metadata consumers can locate the owner
/// through a single `metadata::owner_config_slot()` accessor, without depending on
/// the ownable module directly.
pub static OWNER_CONFIG_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::access::ownable::owner_config")
        .expect("storage slot name should be valid")
});

/// Token name (2 Words = 8 felts), split across 2 slots.
pub static NAME_SLOTS: LazyLock<[StorageSlotName; 2]> = LazyLock::new(|| {
    [
        StorageSlotName::new("miden::standards::metadata::name_0").expect("valid slot name"),
        StorageSlotName::new("miden::standards::metadata::name_1").expect("valid slot name"),
    ]
});

/// Content URI (6 Words = 24 felts), split across 6 slots.
pub static CONTENT_URI_SLOTS: LazyLock<[StorageSlotName; 6]> = LazyLock::new(|| {
    [
        StorageSlotName::new("miden::standards::metadata::content_uri_0").expect("valid slot name"),
        StorageSlotName::new("miden::standards::metadata::content_uri_1").expect("valid slot name"),
        StorageSlotName::new("miden::standards::metadata::content_uri_2").expect("valid slot name"),
        StorageSlotName::new("miden::standards::metadata::content_uri_3").expect("valid slot name"),
        StorageSlotName::new("miden::standards::metadata::content_uri_4").expect("valid slot name"),
        StorageSlotName::new("miden::standards::metadata::content_uri_5").expect("valid slot name"),
    ]
});

/// Schema commitment slot.
pub static SCHEMA_COMMITMENT_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::storage_schema")
        .expect("storage slot name should be valid")
});

// SLOT ACCESSORS
// ================================================================================================

/// Returns the [`StorageSlotName`] for token metadata (slot 0).
pub fn token_metadata_slot() -> &'static StorageSlotName {
    &TOKEN_METADATA_SLOT
}

/// Returns the [`StorageSlotName`] for owner config (slot 1).
pub fn owner_config_slot() -> &'static StorageSlotName {
    &OWNER_CONFIG_SLOT
}

// INFO COMPONENT
// ================================================================================================

/// A metadata component storing name and content URI in fixed value slots.
///
/// ## Storage Layout
///
/// - Slot 2–3: name (2 Words = 8 felts)
/// - Slot 4–9: content_uri (6 Words = 24 felts)
#[derive(Debug, Clone, Default)]
pub struct Info {
    name: Option<[Word; 2]>,
    content_uri: Option<[Word; 6]>,
}

impl Info {
    /// Creates a new empty metadata extension.
    pub fn new() -> Self {
        Self { name: None, content_uri: None }
    }

    /// Sets the name metadata (2 Words).
    pub fn with_name(mut self, name: [Word; 2]) -> Self {
        self.name = Some(name);
        self
    }

    /// Sets the content URI metadata (6 Words).
    pub fn with_content_uri(mut self, content_uri: [Word; 6]) -> Self {
        self.content_uri = Some(content_uri);
        self
    }

    /// Returns the slot name for name chunk 0.
    pub fn name_chunk_0_slot() -> &'static StorageSlotName {
        &NAME_SLOTS[0]
    }

    /// Returns the slot name for name chunk 1.
    pub fn name_chunk_1_slot() -> &'static StorageSlotName {
        &NAME_SLOTS[1]
    }

    /// Returns the slot name for a content URI chunk by index (0..6).
    ///
    /// # Panics
    /// Panics if `index >= 6`.
    pub fn content_uri_slot(index: usize) -> &'static StorageSlotName {
        assert!(index < 6, "content_uri_slot index must be in 0..6, got {index}");
        &CONTENT_URI_SLOTS[index]
    }

    /// Reads the name and content URI from account storage.
    ///
    /// Returns `(name, content_uri)` where each is `Some` only if at least one word is non-zero.
    pub fn read_name_and_content_uri_from_storage(
        storage: &AccountStorage,
    ) -> (Option<[Word; 2]>, Option<[Word; 6]>) {
        // Read name
        let name = if let (Ok(chunk_0), Ok(chunk_1)) = (
            storage.get_item(Info::name_chunk_0_slot()),
            storage.get_item(Info::name_chunk_1_slot()),
        ) {
            let name: [Word; 2] = [chunk_0, chunk_1];
            if name != [Word::default(); 2] { Some(name) } else { None }
        } else {
            None
        };

        // Read content URI
        let mut content_uri = [Word::default(); 6];
        let mut any_set = false;
        for (i, slot) in content_uri.iter_mut().enumerate() {
            if let Ok(chunk) = storage.get_item(Info::content_uri_slot(i)) {
                *slot = chunk;
                if chunk != Word::default() {
                    any_set = true;
                }
            }
        }
        let content_uri = if any_set { Some(content_uri) } else { None };

        (name, content_uri)
    }
}

impl From<Info> for AccountComponent {
    fn from(extension: Info) -> Self {
        let mut storage_slots: Vec<StorageSlot> = Vec::new();

        if let Some(name) = extension.name {
            storage_slots.push(StorageSlot::with_value(Info::name_chunk_0_slot().clone(), name[0]));
            storage_slots.push(StorageSlot::with_value(Info::name_chunk_1_slot().clone(), name[1]));
        }

        if let Some(content_uri) = extension.content_uri {
            for (i, word) in content_uri.iter().enumerate() {
                storage_slots
                    .push(StorageSlot::with_value(Info::content_uri_slot(i).clone(), *word));
            }
        }

        let metadata = AccountComponentMetadata::new("miden::standards::metadata::info")
            .with_description("Metadata info (name, content URI) in fixed value slots")
            .with_supports_all_types();

        AccountComponent::new(metadata_info_component_library(), storage_slots, metadata)
            .expect("Info component should satisfy the requirements")
    }
}

// SCHEMA COMMITMENT COMPONENT
// ================================================================================================

/// An [`AccountComponent`] exposing the account storage schema commitment.
///
/// The [`AccountSchemaCommitment`] component can be constructed from a list of [`StorageSchema`],
/// from which a commitment is computed and then inserted into the [`SCHEMA_COMMITMENT_SLOT_NAME`]
/// slot.
///
/// It reexports the `get_schema_commitment` procedure from
/// `miden::standards::metadata::storage_schema`.
///
/// ## Storage Layout
///
/// - [`Self::schema_commitment_slot`]: Storage schema commitment.
pub struct AccountSchemaCommitment {
    schema_commitment: Word,
}

impl AccountSchemaCommitment {
    /// Creates a new [`AccountSchemaCommitment`] component from a list of storage schemas.
    ///
    /// The input schemas are merged into a single schema before the final commitment is computed.
    ///
    /// # Errors
    ///
    /// Returns an error if the schemas contain conflicting definitions for the same slot name.
    pub fn new(schemas: &[StorageSchema]) -> Result<Self, ComponentMetadataError> {
        Ok(Self {
            schema_commitment: compute_schema_commitment(schemas)?,
        })
    }

    /// Creates a new [`AccountSchemaCommitment`] component from a [`StorageSchema`].
    pub fn from_schema(storage_schema: &StorageSchema) -> Result<Self, ComponentMetadataError> {
        Self::new(core::slice::from_ref(storage_schema))
    }

    /// Returns the [`StorageSlotName`] where the schema commitment is stored.
    pub fn schema_commitment_slot() -> &'static StorageSlotName {
        &SCHEMA_COMMITMENT_SLOT_NAME
    }
}

impl From<AccountSchemaCommitment> for AccountComponent {
    fn from(schema_commitment: AccountSchemaCommitment) -> Self {
        let metadata = AccountComponentMetadata::new("miden::metadata::schema_commitment")
            .with_description("Component exposing the account storage schema commitment")
            .with_supports_all_types();

        AccountComponent::new(
            storage_schema_library(),
            vec![StorageSlot::with_value(
                AccountSchemaCommitment::schema_commitment_slot().clone(),
                schema_commitment.schema_commitment,
            )],
            metadata,
        )
        .expect(
            "AccountSchemaCommitment component should satisfy the requirements of a valid account component",
        )
    }
}

/// Computes the schema commitment.
///
/// The account schema commitment is computed from the merged schema commitment.
/// If the passed list of schemas is empty, [`Word::empty()`] is returned.
fn compute_schema_commitment(schemas: &[StorageSchema]) -> Result<Word, ComponentMetadataError> {
    if schemas.is_empty() {
        return Ok(Word::empty());
    }

    let mut merged_slots = BTreeMap::new();
    for schema in schemas {
        for (slot_name, slot_schema) in schema.iter() {
            match merged_slots.get(slot_name) {
                None => {
                    merged_slots.insert(slot_name.clone(), slot_schema.clone());
                },
                // Slot exists, check if the schema is the same before erroring
                Some(existing) => {
                    if existing != slot_schema {
                        return Err(ComponentMetadataError::InvalidSchema(format!(
                            "conflicting definitions for storage slot `{slot_name}`",
                        )));
                    }
                },
            }
        }
    }

    let merged_schema = StorageSchema::new(merged_slots)?;

    Ok(merged_schema.commitment())
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_protocol::Word;
    use miden_protocol::account::AccountBuilder;
    use miden_protocol::account::component::AccountComponentMetadata;

    use super::{AccountSchemaCommitment, Info};
    use crate::account::auth::NoAuth;

    #[test]
    fn metadata_info_can_store_name_and_content_uri() {
        let name = [Word::from([1u32, 2, 3, 4]), Word::from([5u32, 6, 7, 8])];
        let content_uri = [
            Word::from([10u32, 11, 12, 13]),
            Word::from([14u32, 15, 16, 17]),
            Word::from([18u32, 19, 20, 21]),
            Word::from([22u32, 23, 24, 25]),
            Word::from([26u32, 27, 28, 29]),
            Word::from([30u32, 31, 32, 33]),
        ];

        let extension = Info::new().with_name(name).with_content_uri(content_uri);

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        // Verify name chunks
        let name_0 = account.storage().get_item(Info::name_chunk_0_slot()).unwrap();
        let name_1 = account.storage().get_item(Info::name_chunk_1_slot()).unwrap();
        assert_eq!(name_0, name[0]);
        assert_eq!(name_1, name[1]);

        // Verify content URI chunks
        for (i, expected) in content_uri.iter().enumerate() {
            let chunk = account.storage().get_item(Info::content_uri_slot(i)).unwrap();
            assert_eq!(chunk, *expected);
        }
    }

    #[test]
    fn metadata_info_empty_works() {
        let extension = Info::new();

        let _account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();
    }

    #[test]
    fn metadata_info_name_only_works() {
        let name = [Word::from([1u32, 2, 3, 4]), Word::from([5u32, 6, 7, 8])];
        let extension = Info::new().with_name(name);

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        let name_0 = account.storage().get_item(Info::name_chunk_0_slot()).unwrap();
        let name_1 = account.storage().get_item(Info::name_chunk_1_slot()).unwrap();
        assert_eq!(name_0, name[0]);
        assert_eq!(name_1, name[1]);
    }

    #[test]
    fn storage_schema_commitment_is_order_independent() {
        let toml_a = r#"
            name = "Component A"
            description = "Component A schema"
            version = "0.1.0"
            supported-types = []

            [[storage.slots]]
            name = "test::slot_a"
            type = "word"
        "#;

        let toml_b = r#"
            name = "Component B"
            description = "Component B schema"
            version = "0.1.0"
            supported-types = []

            [[storage.slots]]
            name = "test::slot_b"
            description = "description is committed to"
            type = "word"
        "#;

        let metadata_a = AccountComponentMetadata::from_toml(toml_a).unwrap();
        let metadata_b = AccountComponentMetadata::from_toml(toml_b).unwrap();

        let schema_a = metadata_a.storage_schema().clone();
        let schema_b = metadata_b.storage_schema().clone();

        // Create one component for each of two different accounts, but switch orderings
        let component_a =
            AccountSchemaCommitment::new(&[schema_a.clone(), schema_b.clone()]).unwrap();
        let component_b = AccountSchemaCommitment::new(&[schema_b, schema_a]).unwrap();

        let account_a = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(component_a)
            .build()
            .unwrap();

        let account_b = AccountBuilder::new([2u8; 32])
            .with_auth_component(NoAuth)
            .with_component(component_b)
            .build()
            .unwrap();

        let slot_name = AccountSchemaCommitment::schema_commitment_slot();
        let commitment_a = account_a.storage().get_item(slot_name).unwrap();
        let commitment_b = account_b.storage().get_item(slot_name).unwrap();

        assert_eq!(commitment_a, commitment_b);
    }

    #[test]
    fn storage_schema_commitment_is_empty_for_no_schemas() {
        let component = AccountSchemaCommitment::new(&[]).unwrap();

        assert_eq!(component.schema_commitment, Word::empty());
    }
}
