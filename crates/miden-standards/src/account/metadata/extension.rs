//! Metadata Extension Component
//!
//! Stores account metadata (name, content URI) in fixed value slots.
//! - **Name**: 2 value slots (2 Words = 8 felts)
//! - **Content URI**: 6 value slots (6 Words = 24 felts)
//!
//! # Example
//!
//! ```ignore
//! use miden_standards::account::metadata::MetadataExtension;
//!
//! // Create extension with name and content URI
//! let extension = MetadataExtension::new()
//!     .with_name([name_word_0, name_word_1])
//!     .with_content_uri([uri_0, uri_1, uri_2, uri_3, uri_4, uri_5]);
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
    StorageSlot,
    StorageSlotName,
};
use miden_protocol::utils::sync::LazyLock;

use crate::account::components::metadata_extension_library;

// CONSTANTS
// ================================================================================================

/// The slot name for name chunk 0.
pub static NAME_CHUNK_0_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::name_chunk_0")
        .expect("storage slot name should be valid")
});

/// The slot name for name chunk 1.
pub static NAME_CHUNK_1_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::name_chunk_1")
        .expect("storage slot name should be valid")
});

/// The slot name for content URI chunk 0.
pub static CONTENT_URI_0_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::content_uri_0")
        .expect("storage slot name should be valid")
});

/// The slot name for content URI chunk 1.
pub static CONTENT_URI_1_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::content_uri_1")
        .expect("storage slot name should be valid")
});

/// The slot name for content URI chunk 2.
pub static CONTENT_URI_2_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::content_uri_2")
        .expect("storage slot name should be valid")
});

/// The slot name for content URI chunk 3.
pub static CONTENT_URI_3_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::content_uri_3")
        .expect("storage slot name should be valid")
});

/// The slot name for content URI chunk 4.
pub static CONTENT_URI_4_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::content_uri_4")
        .expect("storage slot name should be valid")
});

/// The slot name for content URI chunk 5.
pub static CONTENT_URI_5_SLOT: LazyLock<StorageSlotName> = LazyLock::new(|| {
    StorageSlotName::new("miden::standards::metadata::content_uri_5")
        .expect("storage slot name should be valid")
});

/// All content URI slot names, indexed 0..5.
pub static CONTENT_URI_SLOTS: LazyLock<[&'static StorageSlotName; 6]> = LazyLock::new(|| {
    [
        &*CONTENT_URI_0_SLOT,
        &*CONTENT_URI_1_SLOT,
        &*CONTENT_URI_2_SLOT,
        &*CONTENT_URI_3_SLOT,
        &*CONTENT_URI_4_SLOT,
        &*CONTENT_URI_5_SLOT,
    ]
});

// METADATA EXTENSION
// ================================================================================================

/// A metadata component storing name and content URI in fixed value slots.
///
/// ## Storage Layout
///
/// - [`NAME_CHUNK_0_SLOT`], [`NAME_CHUNK_1_SLOT`]: Token/account name (2 Words = 8 felts)
/// - [`CONTENT_URI_0_SLOT`] .. [`CONTENT_URI_5_SLOT`]: Content URI (6 Words = 24 felts)
#[derive(Debug, Clone, Default)]
pub struct MetadataExtension {
    name: Option<[Word; 2]>,
    content_uri: Option<[Word; 6]>,
}

impl MetadataExtension {
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
        &NAME_CHUNK_0_SLOT
    }

    /// Returns the slot name for name chunk 1.
    pub fn name_chunk_1_slot() -> &'static StorageSlotName {
        &NAME_CHUNK_1_SLOT
    }

    /// Returns the slot name for a content URI chunk by index (0..5).
    pub fn content_uri_slot(index: usize) -> &'static StorageSlotName {
        CONTENT_URI_SLOTS[index]
    }
}

impl From<MetadataExtension> for AccountComponent {
    fn from(extension: MetadataExtension) -> Self {
        let mut storage_slots: Vec<StorageSlot> = Vec::new();

        if let Some(name) = extension.name {
            storage_slots.push(StorageSlot::with_value(
                MetadataExtension::name_chunk_0_slot().clone(),
                name[0],
            ));
            storage_slots.push(StorageSlot::with_value(
                MetadataExtension::name_chunk_1_slot().clone(),
                name[1],
            ));
        }

        if let Some(content_uri) = extension.content_uri {
            for (i, word) in content_uri.iter().enumerate() {
                storage_slots.push(StorageSlot::with_value(
                    MetadataExtension::content_uri_slot(i).clone(),
                    *word,
                ));
            }
        }

        let metadata = AccountComponentMetadata::new("miden::standards::metadata::extension")
            .with_description("Optional metadata extension (name, content URI) in fixed value slots")
            .with_supports_all_types();

        AccountComponent::new(metadata_extension_library(), storage_slots, metadata)
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
    fn metadata_extension_can_store_name_and_content_uri() {
        let name = [Word::from([1u32, 2, 3, 4]), Word::from([5u32, 6, 7, 8])];
        let content_uri = [
            Word::from([10u32, 11, 12, 13]),
            Word::from([14u32, 15, 16, 17]),
            Word::from([18u32, 19, 20, 21]),
            Word::from([22u32, 23, 24, 25]),
            Word::from([26u32, 27, 28, 29]),
            Word::from([30u32, 31, 32, 33]),
        ];

        let extension = MetadataExtension::new().with_name(name).with_content_uri(content_uri);

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        // Verify name chunks
        let name_0 = account.storage().get_item(MetadataExtension::name_chunk_0_slot()).unwrap();
        let name_1 = account.storage().get_item(MetadataExtension::name_chunk_1_slot()).unwrap();
        assert_eq!(name_0, name[0]);
        assert_eq!(name_1, name[1]);

        // Verify content URI chunks
        for i in 0..6 {
            let chunk = account.storage().get_item(MetadataExtension::content_uri_slot(i)).unwrap();
            assert_eq!(chunk, content_uri[i]);
        }
    }

    #[test]
    fn metadata_extension_empty_works() {
        let extension = MetadataExtension::new();

        let _account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();
    }

    #[test]
    fn metadata_extension_name_only_works() {
        let name = [Word::from([1u32, 2, 3, 4]), Word::from([5u32, 6, 7, 8])];
        let extension = MetadataExtension::new().with_name(name);

        let account = AccountBuilder::new([1u8; 32])
            .with_auth_component(NoAuth)
            .with_component(extension)
            .build()
            .unwrap();

        let name_0 = account.storage().get_item(MetadataExtension::name_chunk_0_slot()).unwrap();
        let name_1 = account.storage().get_item(MetadataExtension::name_chunk_1_slot()).unwrap();
        assert_eq!(name_0, name[0]);
        assert_eq!(name_1, name[1]);
    }
}
