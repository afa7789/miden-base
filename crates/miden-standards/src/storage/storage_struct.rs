use alloc::vec::Vec;

use miden_protocol::{Felt, FieldElement, Word};
use thiserror::Error;

// CONSTANTS
// ================================================================================================

/// Maximum number of fields supported in a single header double-word.
pub const MAX_FIELDS: usize = 4;

// ERRORS
// ================================================================================================

/// Errors related to storage struct operations.
#[derive(Debug, Error)]
pub enum StorageStructError {
    #[error("field_id {field_id} is out of bounds (num_fields: {num_fields})")]
    FieldIdOutOfBounds { field_id: usize, num_fields: u8 },
    #[error("too many fields: {actual}, maximum is {max}")]
    TooManyFields { actual: usize, max: usize },
    #[error("header value {value} does not fit in expected type (max: {max})")]
    ValueOverflow { value: u64, max: u64 },
}

// STORAGE STRUCT HEADER
// ================================================================================================

/// Header for a StorageStruct, stored at index 0 of a double-word array.
///
/// Layout (2 Words = 1 double-word):
/// ```text
///   Word0: [version, num_fields, total_size, flags]
///   Word1: [f0_size, f1_size, f2_size, f3_size]
/// ```
///
/// - `version`: Schema version for future migrations.
/// - `num_fields`: Number of fields in the struct (0 to [`MAX_FIELDS`]).
/// - `total_size`: Sum of all field sizes in double-words.
/// - `flags`: Reserved for future use (currently 0).
/// - `f*_size`: Size of each field in double-words (max 255 each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageStructHeader {
    version: u8,
    num_fields: u8,
    total_size: u16,
    flags: u8,
    field_sizes: [u8; MAX_FIELDS],
}

impl StorageStructHeader {
    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Creates a new header from a version number and field sizes.
    ///
    /// `total_size` is computed automatically as the sum of all field sizes.
    ///
    /// # Errors
    /// Returns an error if more than [`MAX_FIELDS`] field sizes are provided.
    pub fn new(version: u8, field_sizes: &[u8]) -> Result<Self, StorageStructError> {
        if field_sizes.len() > MAX_FIELDS {
            return Err(StorageStructError::TooManyFields {
                actual: field_sizes.len(),
                max: MAX_FIELDS,
            });
        }

        let total_size: u16 = field_sizes.iter().map(|&s| s as u16).sum();
        let num_fields = field_sizes.len() as u8;

        let mut sizes = [0u8; MAX_FIELDS];
        for (i, &s) in field_sizes.iter().enumerate() {
            sizes[i] = s;
        }

        Ok(Self {
            version,
            num_fields,
            total_size,
            flags: 0,
            field_sizes: sizes,
        })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the version of this storage struct.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Returns the number of fields.
    pub fn num_fields(&self) -> u8 {
        self.num_fields
    }

    /// Returns the total size of all fields in double-words.
    pub fn total_size(&self) -> u16 {
        self.total_size
    }

    /// Returns the flags byte.
    pub fn flags(&self) -> u8 {
        self.flags
    }

    /// Returns the active field sizes (only the first `num_fields` entries).
    pub fn field_sizes(&self) -> &[u8] {
        &self.field_sizes[..self.num_fields as usize]
    }

    /// Returns the start index for a field in the double-word array.
    ///
    /// The header occupies index 0, so field data starts at index 1.
    /// Each field's start index is `1 + sum(field_sizes[0..field_id])`.
    ///
    /// # Errors
    /// Returns an error if `field_id >= num_fields`.
    pub fn field_start_index(&self, field_id: usize) -> Result<u64, StorageStructError> {
        if field_id >= self.num_fields as usize {
            return Err(StorageStructError::FieldIdOutOfBounds {
                field_id,
                num_fields: self.num_fields,
            });
        }

        let offset: u64 = self.field_sizes[..field_id].iter().map(|&s| s as u64).sum();
        Ok(1 + offset)
    }

    /// Returns the size of a field in double-words.
    ///
    /// # Errors
    /// Returns an error if `field_id >= num_fields`.
    pub fn field_size(&self, field_id: usize) -> Result<u8, StorageStructError> {
        if field_id >= self.num_fields as usize {
            return Err(StorageStructError::FieldIdOutOfBounds {
                field_id,
                num_fields: self.num_fields,
            });
        }

        Ok(self.field_sizes[field_id])
    }

    /// Serializes the header to 2 Words for storage in a double-word array at index 0.
    pub fn to_words(&self) -> [Word; 2] {
        let word0 = Word::new([
            Felt::new(self.version as u64),
            Felt::new(self.num_fields as u64),
            Felt::new(self.total_size as u64),
            Felt::new(self.flags as u64),
        ]);
        let word1 = Word::new([
            Felt::new(self.field_sizes[0] as u64),
            Felt::new(self.field_sizes[1] as u64),
            Felt::new(self.field_sizes[2] as u64),
            Felt::new(self.field_sizes[3] as u64),
        ]);
        [word0, word1]
    }

    /// Deserializes a header from 2 Words.
    ///
    /// # Errors
    /// Returns an error if any value doesn't fit in its expected type.
    pub fn from_words(words: [Word; 2]) -> Result<Self, StorageStructError> {
        let [w0, w1] = words;
        let [version_felt, num_fields_felt, total_size_felt, flags_felt] = *w0;

        let version = felt_to_u8(version_felt)?;
        let num_fields = felt_to_u8(num_fields_felt)?;
        let total_size = felt_to_u16(total_size_felt)?;
        let flags = felt_to_u8(flags_felt)?;

        if num_fields as usize > MAX_FIELDS {
            return Err(StorageStructError::TooManyFields {
                actual: num_fields as usize,
                max: MAX_FIELDS,
            });
        }

        let [f0, f1, f2, f3] = *w1;
        let field_sizes = [felt_to_u8(f0)?, felt_to_u8(f1)?, felt_to_u8(f2)?, felt_to_u8(f3)?];

        Ok(Self {
            version,
            num_fields,
            total_size,
            flags,
            field_sizes,
        })
    }
}

// HELPER FUNCTIONS
// ================================================================================================

/// Converts double-words (indexed by position) into map entries for a storage map.
///
/// Each double-word at index `i` produces two map entries following the
/// `double_word_array.masm` key convention:
/// - Key `[0, 0, 0, i]` (stack: `[i, 0, 0, 0]`) maps to word0
/// - Key `[0, 0, 1, i]` (stack: `[i, 1, 0, 0]`) maps to word1
pub fn double_words_to_map_entries(double_words: &[[Word; 2]]) -> Vec<(Word, Word)> {
    let mut entries = Vec::with_capacity(double_words.len() * 2);
    for (i, [w0, w1]) in double_words.iter().enumerate() {
        let index = Felt::new(i as u64);
        let key0 = Word::new([Felt::ZERO, Felt::ZERO, Felt::ZERO, index]);
        let key1 = Word::new([Felt::ZERO, Felt::ZERO, Felt::new(1), index]);
        entries.push((key0, *w0));
        entries.push((key1, *w1));
    }
    entries
}

/// Converts a [`Felt`] to a `u8`, returning an error if the value overflows.
fn felt_to_u8(felt: Felt) -> Result<u8, StorageStructError> {
    let value = felt.as_int();
    u8::try_from(value)
        .map_err(|_| StorageStructError::ValueOverflow { value, max: u8::MAX as u64 })
}

/// Converts a [`Felt`] to a `u16`, returning an error if the value overflows.
fn felt_to_u16(felt: Felt) -> Result<u16, StorageStructError> {
    let value = felt.as_int();
    u16::try_from(value)
        .map_err(|_| StorageStructError::ValueOverflow { value, max: u16::MAX as u64 })
}

// TRAITS
// ================================================================================================

/// Trait for reading storage map items, enabling mocking in tests.
pub trait StorageReader {
    /// Reads a single word from a storage map identified by `slot_key` at the given `key`.
    fn get_map_item(&self, slot_key: [Felt; 2], key: Word) -> Result<Word, StorageStructError>;

    /// Reads a double-word from a storage map at the given array index.
    ///
    /// This uses the double_word_array key convention:
    /// - Key `[0, 0, 0, index]` (stack: `[index, 0, 0, 0]`) for the first word
    /// - Key `[0, 0, 1, index]` (stack: `[index, 1, 0, 0]`) for the second word
    fn get_double_word(
        &self,
        slot_key: [Felt; 2],
        index: u64,
    ) -> Result<[Word; 2], StorageStructError> {
        let idx = Felt::new(index);
        let key0 = Word::new([Felt::ZERO, Felt::ZERO, Felt::ZERO, idx]);
        let key1 = Word::new([Felt::ZERO, Felt::ZERO, Felt::new(1), idx]);
        Ok([self.get_map_item(slot_key, key0)?, self.get_map_item(slot_key, key1)?])
    }

    /// Reads the struct header from a storage map.
    fn get_header(&self, slot_key: [Felt; 2]) -> Result<StorageStructHeader, StorageStructError> {
        let words = self.get_double_word(slot_key, 0)?;
        StorageStructHeader::from_words(words)
    }
}

/// Trait for types that can be stored as a StorageStruct.
///
/// Implementors define their schema version, field layout, and serialization logic.
pub trait StorageStruct: Sized {
    /// Schema version for this type.
    const VERSION: u8;

    /// Field sizes in double-words, defining the struct layout.
    fn field_sizes() -> &'static [u8];

    /// Builds the header for this type.
    fn header() -> StorageStructHeader {
        StorageStructHeader::new(Self::VERSION, Self::field_sizes())
            .expect("field_sizes should be valid")
    }

    /// Serializes this value to a sequence of double-words.
    ///
    /// Index 0 should be the header, followed by field data.
    fn to_double_words(&self) -> Vec<[Word; 2]>;

    /// Deserializes from storage using a [`StorageReader`].
    fn from_storage(
        reader: &impl StorageReader,
        slot_key: [Felt; 2],
    ) -> Result<Self, StorageStructError>;
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_protocol::{Felt, Word};

    use super::*;

    #[test]
    fn header_new_basic() {
        let header = StorageStructHeader::new(1, &[1, 1, 4]).unwrap();
        assert_eq!(header.version(), 1);
        assert_eq!(header.num_fields(), 3);
        assert_eq!(header.total_size(), 6);
        assert_eq!(header.flags(), 0);
        assert_eq!(header.field_sizes(), &[1, 1, 4]);
    }

    #[test]
    fn header_new_empty() {
        let header = StorageStructHeader::new(1, &[]).unwrap();
        assert_eq!(header.num_fields(), 0);
        assert_eq!(header.total_size(), 0);
        assert_eq!(header.field_sizes(), &[]);
    }

    #[test]
    fn header_new_max_fields() {
        let header = StorageStructHeader::new(1, &[1, 2, 3, 4]).unwrap();
        assert_eq!(header.num_fields(), 4);
        assert_eq!(header.total_size(), 10);
        assert_eq!(header.field_sizes(), &[1, 2, 3, 4]);
    }

    #[test]
    fn header_new_too_many_fields() {
        let result = StorageStructHeader::new(1, &[1, 2, 3, 4, 5]);
        assert!(matches!(result, Err(StorageStructError::TooManyFields { actual: 5, max: 4 })));
    }

    #[test]
    fn header_roundtrip() {
        let original = StorageStructHeader::new(1, &[1, 1, 4]).unwrap();
        let words = original.to_words();
        let restored = StorageStructHeader::from_words(words).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn header_roundtrip_max_fields() {
        let original = StorageStructHeader::new(2, &[10, 20, 30, 40]).unwrap();
        let words = original.to_words();
        let restored = StorageStructHeader::from_words(words).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn header_roundtrip_empty() {
        let original = StorageStructHeader::new(1, &[]).unwrap();
        let words = original.to_words();
        let restored = StorageStructHeader::from_words(words).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn header_field_start_indices() {
        // Layout: field0(1dw), field1(1dw), field2(4dw)
        let header = StorageStructHeader::new(1, &[1, 1, 4]).unwrap();

        assert_eq!(header.field_start_index(0).unwrap(), 1);
        assert_eq!(header.field_start_index(1).unwrap(), 2);
        assert_eq!(header.field_start_index(2).unwrap(), 3);
    }

    #[test]
    fn header_field_start_indices_varied() {
        // Layout: field0(2dw), field1(3dw), field2(1dw), field3(4dw)
        let header = StorageStructHeader::new(1, &[2, 3, 1, 4]).unwrap();

        assert_eq!(header.field_start_index(0).unwrap(), 1);
        assert_eq!(header.field_start_index(1).unwrap(), 3); // 1 + 2
        assert_eq!(header.field_start_index(2).unwrap(), 6); // 1 + 2 + 3
        assert_eq!(header.field_start_index(3).unwrap(), 7); // 1 + 2 + 3 + 1
    }

    #[test]
    fn header_field_start_index_out_of_bounds() {
        let header = StorageStructHeader::new(1, &[1, 1]).unwrap();
        assert!(matches!(
            header.field_start_index(2),
            Err(StorageStructError::FieldIdOutOfBounds { field_id: 2, num_fields: 2 })
        ));
    }

    #[test]
    fn header_field_sizes_access() {
        let header = StorageStructHeader::new(1, &[1, 2, 3]).unwrap();

        assert_eq!(header.field_size(0).unwrap(), 1);
        assert_eq!(header.field_size(1).unwrap(), 2);
        assert_eq!(header.field_size(2).unwrap(), 3);
    }

    #[test]
    fn header_field_size_out_of_bounds() {
        let header = StorageStructHeader::new(1, &[1]).unwrap();
        assert!(matches!(
            header.field_size(1),
            Err(StorageStructError::FieldIdOutOfBounds { field_id: 1, num_fields: 1 })
        ));
    }

    #[test]
    fn header_to_words_layout() {
        let header = StorageStructHeader::new(1, &[1, 2, 3]).unwrap();
        let [w0, w1] = header.to_words();

        // Word0: [version, num_fields, total_size, flags]
        assert_eq!(w0[0], Felt::new(1)); // version
        assert_eq!(w0[1], Felt::new(3)); // num_fields
        assert_eq!(w0[2], Felt::new(6)); // total_size = 1+2+3
        assert_eq!(w0[3], Felt::ZERO); // flags

        // Word1: [f0_size, f1_size, f2_size, f3_size]
        assert_eq!(w1[0], Felt::new(1));
        assert_eq!(w1[1], Felt::new(2));
        assert_eq!(w1[2], Felt::new(3));
        assert_eq!(w1[3], Felt::ZERO); // unused field
    }

    #[test]
    fn from_words_overflow_detection() {
        // Create words with a value that doesn't fit in u8
        let w0 = Word::new([
            Felt::new(256), // version > u8::MAX
            Felt::ZERO,
            Felt::ZERO,
            Felt::ZERO,
        ]);
        let w1 = Word::new([Felt::ZERO; 4]);

        assert!(matches!(
            StorageStructHeader::from_words([w0, w1]),
            Err(StorageStructError::ValueOverflow { value: 256, max: 255 })
        ));
    }

    #[test]
    fn from_words_too_many_fields() {
        // num_fields in header > MAX_FIELDS
        let w0 = Word::new([
            Felt::new(1), // version
            Felt::new(5), // num_fields > 4
            Felt::ZERO,
            Felt::ZERO,
        ]);
        let w1 = Word::new([Felt::ZERO; 4]);

        assert!(matches!(
            StorageStructHeader::from_words([w0, w1]),
            Err(StorageStructError::TooManyFields { actual: 5, max: 4 })
        ));
    }

    #[test]
    fn from_words_total_size_u16_overflow() {
        // total_size that doesn't fit in u16
        let w0 = Word::new([
            Felt::new(1),     // version
            Felt::new(2),     // num_fields
            Felt::new(65536), // total_size > u16::MAX
            Felt::ZERO,
        ]);
        let w1 = Word::new([Felt::new(1), Felt::new(1), Felt::ZERO, Felt::ZERO]);

        assert!(matches!(
            StorageStructHeader::from_words([w0, w1]),
            Err(StorageStructError::ValueOverflow { value: 65536, max: 65535 })
        ));
    }

    #[test]
    fn double_words_to_map_entries_basic() {
        let dw0 = [
            Word::new([Felt::new(1), Felt::new(2), Felt::new(3), Felt::ZERO]),
            Word::new([Felt::new(10), Felt::new(20), Felt::ZERO, Felt::ZERO]),
        ];
        let dw1 = [
            Word::new([Felt::new(100), Felt::ZERO, Felt::ZERO, Felt::ZERO]),
            Word::new([Felt::new(200), Felt::ZERO, Felt::ZERO, Felt::ZERO]),
        ];

        let entries = double_words_to_map_entries(&[dw0, dw1]);
        assert_eq!(entries.len(), 4);

        // Index 0, word 0: key Word=[0,0,0,0] (stack: [0,0,0,0])
        assert_eq!(entries[0].0, Word::new([Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::ZERO]));
        assert_eq!(entries[0].1, dw0[0]);

        // Index 0, word 1: key Word=[0,0,1,0] (stack: [0,1,0,0])
        assert_eq!(entries[1].0, Word::new([Felt::ZERO, Felt::ZERO, Felt::new(1), Felt::ZERO]));
        assert_eq!(entries[1].1, dw0[1]);

        // Index 1, word 0: key Word=[0,0,0,1] (stack: [1,0,0,0])
        assert_eq!(entries[2].0, Word::new([Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::new(1)]));
        assert_eq!(entries[2].1, dw1[0]);

        // Index 1, word 1: key Word=[0,0,1,1] (stack: [1,1,0,0])
        assert_eq!(entries[3].0, Word::new([Felt::ZERO, Felt::ZERO, Felt::new(1), Felt::new(1)]));
        assert_eq!(entries[3].1, dw1[1]);
    }

    // MOCK STORAGE
    // --------------------------------------------------------------------------------------------

    /// Mock storage reader for testing without real accounts.
    struct MockStorage {
        /// Double-words indexed by position (ignores slot_key).
        data: Vec<[Word; 2]>,
    }

    impl MockStorage {
        fn from_double_words(data: Vec<[Word; 2]>) -> Self {
            Self { data }
        }
    }

    impl StorageReader for MockStorage {
        fn get_map_item(
            &self,
            _slot_key: [Felt; 2],
            key: Word,
        ) -> Result<Word, StorageStructError> {
            // Keys follow the double_word_array convention:
            // key[3] = index (top of stack), key[2] = sub_index (0 or 1)
            let index = key[3].as_int() as usize;
            let sub_index = key[2].as_int() as usize;

            if index >= self.data.len() {
                return Ok(Word::new([Felt::ZERO; 4]));
            }

            match sub_index {
                0 => Ok(self.data[index][0]),
                1 => Ok(self.data[index][1]),
                _ => Ok(Word::new([Felt::ZERO; 4])),
            }
        }
    }

    #[test]
    fn storage_reader_get_header() {
        let header = StorageStructHeader::new(1, &[1, 1, 4]).unwrap();
        let [w0, w1] = header.to_words();

        let storage = MockStorage::from_double_words(vec![[w0, w1]]);
        let slot_key = [Felt::ZERO, Felt::ZERO];

        let restored = storage.get_header(slot_key).unwrap();
        assert_eq!(header, restored);
    }

    #[test]
    fn storage_reader_get_double_word() {
        let field_data = [
            Word::new([Felt::new(42), Felt::new(43), Felt::ZERO, Felt::ZERO]),
            Word::new([Felt::new(44), Felt::new(45), Felt::ZERO, Felt::ZERO]),
        ];

        let header = StorageStructHeader::new(1, &[1]).unwrap();
        let [hw0, hw1] = header.to_words();

        let storage = MockStorage::from_double_words(vec![[hw0, hw1], field_data]);
        let slot_key = [Felt::ZERO, Felt::ZERO];

        let [w0, w1] = storage.get_double_word(slot_key, 1).unwrap();
        assert_eq!(w0, field_data[0]);
        assert_eq!(w1, field_data[1]);
    }

    #[test]
    fn mock_storage_full_struct_read() {
        // Simulate a struct with 3 fields: core(1dw), name(1dw), uri(4dw)
        let header = StorageStructHeader::new(1, &[1, 1, 4]).unwrap();
        let [hw0, hw1] = header.to_words();

        let core_data = [
            Word::new([Felt::new(100), Felt::new(200), Felt::new(8), Felt::new(999)]),
            Word::new([Felt::ZERO; 4]),
        ];
        let name_data = [
            Word::new([Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)]),
            Word::new([Felt::new(5), Felt::new(6), Felt::new(7), Felt::new(8)]),
        ];
        let uri_chunk0 = [
            Word::new([Felt::new(10), Felt::ZERO, Felt::ZERO, Felt::ZERO]),
            Word::new([Felt::new(11), Felt::ZERO, Felt::ZERO, Felt::ZERO]),
        ];
        let uri_chunk1 = [
            Word::new([Felt::new(20), Felt::ZERO, Felt::ZERO, Felt::ZERO]),
            Word::new([Felt::new(21), Felt::ZERO, Felt::ZERO, Felt::ZERO]),
        ];
        let uri_chunk2 = [Word::new([Felt::ZERO; 4]); 2];
        let uri_chunk3 = [Word::new([Felt::ZERO; 4]); 2];

        let storage = MockStorage::from_double_words(vec![
            [hw0, hw1], // index 0: header
            core_data,  // index 1: core field
            name_data,  // index 2: name field
            uri_chunk0, // index 3: uri chunk 0
            uri_chunk1, // index 4: uri chunk 1
            uri_chunk2, // index 5: uri chunk 2
            uri_chunk3, // index 6: uri chunk 3
        ]);
        let slot_key = [Felt::ZERO, Felt::ZERO];

        // Verify header
        let h = storage.get_header(slot_key).unwrap();
        assert_eq!(h.num_fields(), 3);
        assert_eq!(h.field_start_index(0).unwrap(), 1);
        assert_eq!(h.field_start_index(1).unwrap(), 2);
        assert_eq!(h.field_start_index(2).unwrap(), 3);

        // Verify field reads using start indices
        let core_idx = h.field_start_index(0).unwrap();
        let [w0, _] = storage.get_double_word(slot_key, core_idx).unwrap();
        assert_eq!(w0[0], Felt::new(100));

        let name_idx = h.field_start_index(1).unwrap();
        let [w0, w1] = storage.get_double_word(slot_key, name_idx).unwrap();
        assert_eq!(w0, name_data[0]);
        assert_eq!(w1, name_data[1]);

        // URI field: read chunk 1 (second chunk of the 4-chunk URI field)
        let uri_start = h.field_start_index(2).unwrap();
        let [w0, w1] = storage.get_double_word(slot_key, uri_start + 1).unwrap();
        assert_eq!(w0, uri_chunk1[0]);
        assert_eq!(w1, uri_chunk1[1]);
    }

    // StorageStruct trait roundtrip
    // --------------------------------------------------------------------------------------------

    /// Minimal test struct: 2 fields of 1 double-word each.
    #[derive(Debug, PartialEq, Eq)]
    struct TestStruct {
        field0: [Word; 2],
        field1: [Word; 2],
    }

    impl StorageStruct for TestStruct {
        const VERSION: u8 = 1;

        fn field_sizes() -> &'static [u8] {
            &[1, 1]
        }

        fn to_double_words(&self) -> Vec<[Word; 2]> {
            vec![Self::header().to_words(), self.field0, self.field1]
        }

        fn from_storage(
            reader: &impl StorageReader,
            slot_key: [Felt; 2],
        ) -> Result<Self, StorageStructError> {
            let header = reader.get_header(slot_key)?;
            let idx0 = header.field_start_index(0)?;
            let idx1 = header.field_start_index(1)?;
            let field0 = reader.get_double_word(slot_key, idx0)?;
            let field1 = reader.get_double_word(slot_key, idx1)?;
            Ok(Self {
                field0: [field0[0], field0[1]],
                field1: [field1[0], field1[1]],
            })
        }
    }

    #[test]
    fn storage_struct_trait_roundtrip() {
        let value = TestStruct {
            field0: [
                Word::new([Felt::new(1), Felt::new(2), Felt::ZERO, Felt::ZERO]),
                Word::new([Felt::new(3), Felt::new(4), Felt::ZERO, Felt::ZERO]),
            ],
            field1: [
                Word::new([Felt::new(10), Felt::new(20), Felt::ZERO, Felt::ZERO]),
                Word::new([Felt::new(30), Felt::new(40), Felt::ZERO, Felt::ZERO]),
            ],
        };

        let double_words = value.to_double_words();
        assert_eq!(double_words.len(), 3); // header + 2 fields

        let storage = MockStorage::from_double_words(double_words);
        let slot_key = [Felt::ZERO, Felt::ZERO];

        let restored = TestStruct::from_storage(&storage, slot_key).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn double_words_to_map_entries_empty() {
        let entries = double_words_to_map_entries(&[]);
        assert!(entries.is_empty());
    }
}
