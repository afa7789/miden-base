//! Tests for the StorageStruct utility procedures.
//!
//! These tests verify that the MASM `storage_struct` procedures correctly read
//! self-describing struct headers and field data from a double-word array.

use miden_protocol::Word;
use miden_protocol::account::{
    AccountBuilder,
    AccountComponent,
    AccountComponentMetadata,
    StorageMap,
    StorageSlot,
    StorageSlotName,
};
use miden_standards::code_builder::CodeBuilder;
use miden_standards::storage::{StorageStructHeader, double_words_to_map_entries};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use crate::{Auth, TransactionContextBuilder};

/// The slot name used for testing the storage struct component.
const TEST_STORAGE_STRUCT_SLOT: &str = "test::storage_struct::data";

/// Test that `get_header` correctly reads the struct header from index 0 (empty struct, 0 fields).
#[tokio::test]
async fn test_storage_struct_get_header_empty_fields() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        pub proc test_get_header
            push.STRUCT_SLOT[0..2]
            exec.storage_struct::get_header
            swapdw dropw dropw
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: version=2, fields=[] (0 fields, total_size=0)
    let header = StorageStructHeader::new(2, &[]).unwrap();
    let [hw0, hw1] = header.to_words();

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[[hw0, hw1]]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = format!(
        r#"
        use wrapper::component->wrapper

        begin
            call.wrapper::test_get_header
            push.{hw0}
            assert_eqw.err="get_header VALUE_0 mismatch (empty)"
            push.{hw1}
            assert_eqw.err="get_header VALUE_1 mismatch (empty)"
        end
        "#,
    );

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that `get_header` correctly reads the struct header from index 0.
#[tokio::test]
async fn test_storage_struct_get_header() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        pub proc test_get_header
            push.STRUCT_SLOT[0..2]
            exec.storage_struct::get_header
            # => [VALUE_0, VALUE_1, pad...]
            # VALUE_0 (top): header metadata
            # VALUE_1 (bottom): field sizes
            swapdw dropw dropw
            # => [VALUE_0, VALUE_1, pad]
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Create header: version=1, fields=[1, 2, 3] (3 fields, total_size=6)
    let header = StorageStructHeader::new(1, &[1, 2, 3]).unwrap();
    let [hw0, hw1] = header.to_words();

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[[hw0, hw1]]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = format!(
        r#"
        use wrapper::component->wrapper

        begin
            call.wrapper::test_get_header

            # VALUE_0 (header metadata) is on top
            push.{hw0}
            assert_eqw.err="get_header VALUE_0 mismatch"

            # VALUE_1 (field sizes) is next
            push.{hw1}
            assert_eqw.err="get_header VALUE_1 mismatch"
        end
        "#,
    );

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that `get_field_start_index` computes correct start indices for all fields.
#[tokio::test]
async fn test_storage_struct_get_field_start_index() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        #! Input: [field_id, pad(15)]
        #! Output: [start_index, pad(15)]
        pub proc test_get_field_start_index
            push.STRUCT_SLOT[0..2]
            movup.2
            # => [field_id, slot_id_prefix, slot_id_suffix, pad...]
            exec.storage_struct::get_field_start_index
            # => [start_index, pad...]
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: fields=[2, 3, 1] -> start indices: field0=1, field1=3, field2=6
    let header = StorageStructHeader::new(1, &[2, 3, 1]).unwrap();
    let [hw0, hw1] = header.to_words();

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[[hw0, hw1]]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = r#"
        use wrapper::component->wrapper

        begin
            # Field 0: start_index = 1
            push.0
            call.wrapper::test_get_field_start_index
            push.1
            assert_eq.err="field 0 start_index should be 1"

            # Field 1: start_index = 1 + 2 = 3
            push.1
            call.wrapper::test_get_field_start_index
            push.3
            assert_eq.err="field 1 start_index should be 3"

            # Field 2: start_index = 1 + 2 + 3 = 6
            push.2
            call.wrapper::test_get_field_start_index
            push.6
            assert_eq.err="field 2 start_index should be 6"
        end
        "#;

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that `get_field_size` returns correct sizes for each field.
#[tokio::test]
async fn test_storage_struct_get_field_size() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        #! Input: [field_id, pad(15)]
        #! Output: [field_size, pad(15)]
        pub proc test_get_field_size
            push.STRUCT_SLOT[0..2]
            movup.2
            exec.storage_struct::get_field_size
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: fields=[5, 1, 3, 2]
    let header = StorageStructHeader::new(1, &[5, 1, 3, 2]).unwrap();
    let [hw0, hw1] = header.to_words();

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[[hw0, hw1]]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = r#"
        use wrapper::component->wrapper

        begin
            push.0
            call.wrapper::test_get_field_size
            push.5
            assert_eq.err="field 0 size should be 5"

            push.1
            call.wrapper::test_get_field_size
            push.1
            assert_eq.err="field 1 size should be 1"

            push.2
            call.wrapper::test_get_field_size
            push.3
            assert_eq.err="field 2 size should be 3"

            push.3
            call.wrapper::test_get_field_size
            push.2
            assert_eq.err="field 3 size should be 2"
        end
        "#;

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that `get_field` reads the correct data for each field.
#[tokio::test]
async fn test_storage_struct_get_field() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        #! Input: [field_id, pad(15)]
        #! Output: [VALUE_0, VALUE_1, pad(8)]
        pub proc test_get_field
            push.STRUCT_SLOT[0..2]
            movup.2
            exec.storage_struct::get_field
            # => [VALUE_0, VALUE_1, pad...]
            swapdw dropw dropw
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: fields=[1, 1] (2 fields, each 1 double-word)
    let header = StorageStructHeader::new(1, &[1, 1]).unwrap();
    let [hw0, hw1] = header.to_words();

    let field0_w0 = Word::from([10u32, 20, 30, 40]);
    let field0_w1 = Word::from([50u32, 60, 70, 80]);
    let field1_w0 = Word::from([100u32, 200, 0, 0]);
    let field1_w1 = Word::from([0u32, 0, 0, 0]);

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[
                [hw0, hw1],             // index 0: header
                [field0_w0, field0_w1], // index 1: field 0
                [field1_w0, field1_w1], // index 2: field 1
            ]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = format!(
        r#"
        use wrapper::component->wrapper

        begin
            # Read field 0
            push.0
            call.wrapper::test_get_field

            push.{field0_w0}
            assert_eqw.err="field 0 word 0 mismatch"

            push.{field0_w1}
            assert_eqw.err="field 0 word 1 mismatch"

            # Read field 1
            push.1
            call.wrapper::test_get_field

            push.{field1_w0}
            assert_eqw.err="field 1 word 0 mismatch"

            push.{field1_w1}
            assert_eqw.err="field 1 word 1 mismatch"
        end
        "#,
    );

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that `get_field` reads the correct data for all three fields (3-field struct).
#[tokio::test]
async fn test_storage_struct_get_field_three_fields() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        pub proc test_get_field
            push.STRUCT_SLOT[0..2]
            movup.2
            exec.storage_struct::get_field
            swapdw dropw dropw
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: fields=[1, 1, 1] (3 fields, each 1 double-word)
    let header = StorageStructHeader::new(1, &[1, 1, 1]).unwrap();
    let [hw0, hw1] = header.to_words();

    let f0_w0 = Word::from([1u32, 2, 3, 4]);
    let f0_w1 = Word::from([5u32, 6, 7, 8]);
    let f1_w0 = Word::from([10u32, 20, 30, 40]);
    let f1_w1 = Word::from([50u32, 60, 70, 80]);
    let f2_w0 = Word::from([100u32, 200, 0, 0]);
    let f2_w1 = Word::from([0u32, 0, 0, 0]);

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[
                [hw0, hw1],
                [f0_w0, f0_w1],
                [f1_w0, f1_w1],
                [f2_w0, f2_w1],
            ]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = format!(
        r#"
        use wrapper::component->wrapper

        begin
            push.0
            call.wrapper::test_get_field
            push.{f0_w0}
            assert_eqw.err="field 0 word 0 mismatch"
            push.{f0_w1}
            assert_eqw.err="field 0 word 1 mismatch"

            push.1
            call.wrapper::test_get_field
            push.{f1_w0}
            assert_eqw.err="field 1 word 0 mismatch"
            push.{f1_w1}
            assert_eqw.err="field 1 word 1 mismatch"

            push.2
            call.wrapper::test_get_field
            push.{f2_w0}
            assert_eqw.err="field 2 word 0 mismatch"
            push.{f2_w1}
            assert_eqw.err="field 2 word 1 mismatch"
        end
        "#,
    );

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that `get_field_chunk` correctly reads specific chunks of multi-double-word fields.
#[tokio::test]
async fn test_storage_struct_get_field_chunk() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        #! Input: [field_id, chunk_index, pad(14)]
        #! Output: [VALUE_0, VALUE_1, pad(8)]
        pub proc test_get_field_chunk
            push.STRUCT_SLOT[0..2]
            # => [prefix, suffix, field_id, chunk_index, ...]
            movup.3 movup.3
            # => [field_id (top), chunk_index, slot_id_prefix, slot_id_suffix]
            exec.storage_struct::get_field_chunk
            swapdw dropw dropw
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: fields=[1, 3] (field 0 = 1dw, field 1 = 3dw)
    let header = StorageStructHeader::new(1, &[1, 3]).unwrap();
    let [hw0, hw1] = header.to_words();

    let f0_w0 = Word::from([1u32, 1, 1, 1]);
    let f0_w1 = Word::from([2u32, 2, 2, 2]);

    // Field 1 has 3 chunks
    let f1_chunk0_w0 = Word::from([10u32, 11, 12, 13]);
    let f1_chunk0_w1 = Word::from([14u32, 15, 16, 17]);
    let f1_chunk1_w0 = Word::from([20u32, 21, 22, 23]);
    let f1_chunk1_w1 = Word::from([24u32, 25, 26, 27]);
    let f1_chunk2_w0 = Word::from([30u32, 31, 32, 33]);
    let f1_chunk2_w1 = Word::from([34u32, 35, 36, 37]);

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[
                [hw0, hw1],                   // index 0: header
                [f0_w0, f0_w1],               // index 1: field 0
                [f1_chunk0_w0, f1_chunk0_w1], // index 2: field 1, chunk 0
                [f1_chunk1_w0, f1_chunk1_w1], // index 3: field 1, chunk 1
                [f1_chunk2_w0, f1_chunk2_w1], // index 4: field 1, chunk 2
            ]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    let tx_script_code = format!(
        r#"
        use wrapper::component->wrapper

        begin
            # Read field 1, chunk 0
            push.0 push.1
            call.wrapper::test_get_field_chunk
            push.{f1_chunk0_w0}
            assert_eqw.err="field 1 chunk 0 word 0 mismatch"
            push.{f1_chunk0_w1}
            assert_eqw.err="field 1 chunk 0 word 1 mismatch"

            # Read field 1, chunk 1
            push.1 push.1
            call.wrapper::test_get_field_chunk
            push.{f1_chunk1_w0}
            assert_eqw.err="field 1 chunk 1 word 0 mismatch"
            push.{f1_chunk1_w1}
            assert_eqw.err="field 1 chunk 1 word 1 mismatch"

            # Read field 1, chunk 2
            push.2 push.1
            call.wrapper::test_get_field_chunk
            push.{f1_chunk2_w0}
            assert_eqw.err="field 1 chunk 2 word 0 mismatch"
            push.{f1_chunk2_w1}
            assert_eqw.err="field 1 chunk 2 word 1 mismatch"

            # Read field 0, chunk 0 (single-chunk field)
            push.0 push.0
            call.wrapper::test_get_field_chunk
            push.{f0_w0}
            assert_eqw.err="field 0 chunk 0 word 0 mismatch"
            push.{f0_w1}
            assert_eqw.err="field 0 chunk 0 word 1 mismatch"
        end
        "#,
    );

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}

/// Test that MASM field start indices match Rust calculations for all 4 fields.
#[tokio::test]
async fn test_storage_struct_indices_match_rust() -> anyhow::Result<()> {
    let slot_name =
        StorageSlotName::new(TEST_STORAGE_STRUCT_SLOT).expect("slot name should be valid");

    let wrapper_component_code = format!(
        r#"
        use miden::core::word
        use miden::standards::data_structures::storage_struct

        const STRUCT_SLOT = word("{slot_name}")

        pub proc test_get_field_start_index
            push.STRUCT_SLOT[0..2]
            movup.2
            exec.storage_struct::get_field_start_index
        end
        "#,
    );

    let wrapper_library = CodeBuilder::default()
        .compile_component_code("wrapper::component", wrapper_component_code)?;

    // Header: fields=[2, 3, 1, 4] (4 fields)
    let header = StorageStructHeader::new(1, &[2, 3, 1, 4]).unwrap();
    let [hw0, hw1] = header.to_words();

    let metadata = AccountComponentMetadata::new("test::storage_struct").with_supports_all_types();
    let wrapper_component = AccountComponent::new(
        wrapper_library.clone(),
        vec![StorageSlot::with_map(
            slot_name.clone(),
            StorageMap::with_entries(double_words_to_map_entries(&[[hw0, hw1]]))?,
        )],
        metadata,
    )?;

    let account = AccountBuilder::new(ChaCha20Rng::from_os_rng().random())
        .with_auth_component(Auth::IncrNonce)
        .with_component(wrapper_component)
        .build_existing()?;

    // Compute expected indices from Rust
    let expected_0 = header.field_start_index(0).unwrap();
    let expected_1 = header.field_start_index(1).unwrap();
    let expected_2 = header.field_start_index(2).unwrap();
    let expected_3 = header.field_start_index(3).unwrap();

    let tx_script_code = format!(
        r#"
        use wrapper::component->wrapper

        begin
            push.0
            call.wrapper::test_get_field_start_index
            push.{expected_0}
            assert_eq.err="field 0 start_index mismatch with Rust"

            push.1
            call.wrapper::test_get_field_start_index
            push.{expected_1}
            assert_eq.err="field 1 start_index mismatch with Rust"

            push.2
            call.wrapper::test_get_field_start_index
            push.{expected_2}
            assert_eq.err="field 2 start_index mismatch with Rust"

            push.3
            call.wrapper::test_get_field_start_index
            push.{expected_3}
            assert_eq.err="field 3 start_index mismatch with Rust"
        end
        "#,
    );

    let tx_script = CodeBuilder::default()
        .with_dynamically_linked_library(&wrapper_library)?
        .compile_tx_script(tx_script_code)?;

    let tx_context = TransactionContextBuilder::new(account).tx_script(tx_script).build()?;
    tx_context.execute().await?;

    Ok(())
}
