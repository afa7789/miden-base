//! Metadata Extension Component
//!
//! A flexible key-value store for account metadata using a StorageMap slot.
//! Any Word can be used as a key to store and retrieve a Word value.
//!
//! # Example
//!
//! ```ignore
//! use miden_standards::account::metadata::MetadataExtension;
//!
//! // Create extension with name and URI
//! let extension = MetadataExtension::new()
//!     .with_entry(MetadataExtension::key_name(), name_word)
//!     .with_entry(MetadataExtension::key_uri(), uri_word);
//!
//! // Add to account
//! let account = AccountBuilder::new(seed)
//!     .with_component(extension)
//!     .build()?;
//! ```

use alloc::vec::Vec;

use miden_protocol::Word;
use miden_protocol::account::{
    AccountComponent,
    AccountComponentMetadata,
    StorageMap,
    StorageSlot,
    StorageSlotName,
};
use miden_protocol::utils::hash_string_to_word;
use miden_protocol::utils::sync::LazyLock;

use crate::account::components::metadata_extension_library;

// CONSTANTS
// ================================================================================================

/// The slot name for the metadata extension StorageMap.
pub static METADATA_EXTENSION_SLOT_NAME: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::extension")
        .expect("storage slot name should be valid")
});

/// Pre-defined key for token/account name.
pub static KEY_NAME: LazyLock<Word> =
    LazyLock::new(|| hash_string_to_word("miden::standards::metadata::extension::key::name"));

/// Pre-defined key for URI (metadata URL, IPFS hash, etc).
pub static KEY_URI: LazyLock<Word> =
    LazyLock::new(|| hash_string_to_word("miden::standards::metadata::extension::key::uri"));

// METADATA EXTENSION
// ================================================================================================

/// A flexible key-value metadata store using a StorageMap.
///
/// This component provides a single StorageMap slot that can hold an unlimited number
/// of key-value pairs. Each key and value is a Word (4 field elements).
///
/// ## Storage Layout
///
/// - [`METADATA_EXTENSION_SLOT_NAME`]: StorageMap containing all metadata entries.
///
/// ## Pre-defined Keys
///
/// - [`KEY_NAME`]: Token/account name
/// - [`KEY_URI`]: URI for external metadata
///
/// Custom keys can be used by computing a Word from any unique identifier.
#[derive(Debug, Clone, Default)]
pub struct MetadataExtension {
    entries: Vec<(Word, Word)>,
}

impl MetadataExtension {
    /// Creates a new empty metadata extension.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Adds a key-value entry to the metadata.
    pub fn with_entry(mut self, key: Word, value: Word) -> Self {
        self.entries.push((key, value));
        self
    }

    /// Adds the name metadata entry.
    pub fn with_name(self, name: Word) -> Self {
        self.with_entry(*KEY_NAME, name)
    }

    /// Adds the URI metadata entry.
    pub fn with_uri(self, uri: Word) -> Self {
        self.with_entry(*KEY_URI, uri)
    }

    /// Returns the slot name for the metadata extension.
    pub fn slot() -> &'static StorageSlotName {
        &METADATA_EXTENSION_SLOT_NAME
    }

    /// Returns the pre-defined key for name.
    pub fn key_name() -> Word {
        *KEY_NAME
    }

    /// Returns the pre-defined key for URI.
    pub fn key_uri() -> Word {
        *KEY_URI
    }
}

impl From<MetadataExtension> for AccountComponent {
    fn from(extension: MetadataExtension) -> Self {
        let storage_map = if extension.entries.is_empty() {
            StorageMap::new()
        } else {
            StorageMap::with_entries(extension.entries)
                .expect("metadata extension entries should not have duplicate keys")
        };

        let metadata = AccountComponentMetadata::new("miden::standards::metadata::extension")
            .with_description("Optional key-value metadata extension (name, URI, etc.)")
            .with_supports_all_types();

        AccountComponent::new(
            metadata_extension_library(),
            vec![StorageSlot::with_map(MetadataExtension::slot().clone(), storage_map)],
            metadata,
        )
        .expect("MetadataExtension component should satisfy the requirements")
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_protocol::Word;
    use miden_protocol::account::AccountBuilder;

    use super::MetadataExtension;
    use crate::account::auth::NoAuth;

    #[test]
    fn metadata_extension_can_store_name_and_uri() {
        let name_value = Word::from([1u32, 2, 3, 4]);
        let uri_value = Word::from([5u32, 6, 7, 8]);

        let extension = MetadataExtension::new().with_name(name_value).with_uri(uri_value);

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        // Verify the storage map was created
        let slot = MetadataExtension::slot();
        let name_from_storage =
            account.storage().get_map_item(slot, MetadataExtension::key_name()).unwrap();
        let uri_from_storage =
            account.storage().get_map_item(slot, MetadataExtension::key_uri()).unwrap();

        assert_eq!(name_from_storage, name_value);
        assert_eq!(uri_from_storage, uri_value);
    }

    #[test]
    fn metadata_extension_empty_works() {
        let extension = MetadataExtension::new();

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        // Empty map should return empty word for any key
        let slot = MetadataExtension::slot();
        let value = account.storage().get_map_item(slot, MetadataExtension::key_name()).unwrap();

        assert_eq!(value, Word::default());
    }

    #[test]
    fn metadata_extension_custom_key_works() {
        let custom_key = Word::from([100u32, 200, 300, 400]);
        let custom_value = Word::from([1u32, 2, 3, 4]);

        let extension = MetadataExtension::new().with_entry(custom_key, custom_value);

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        let slot = MetadataExtension::slot();
        let value = account.storage().get_map_item(slot, custom_key).unwrap();

        assert_eq!(value, custom_value);
    }
}
