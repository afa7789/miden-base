//! Integration tests for the Metadata Extension component.

extern crate alloc;

use alloc::sync::Arc;

use miden_protocol::Word;
use miden_protocol::account::AccountBuilder;
use miden_protocol::assembly::DefaultSourceManager;
use miden_standards::account::auth::NoAuth;
use miden_standards::account::metadata::MetadataExtension;
use miden_standards::code_builder::CodeBuilder;
use miden_testing::TransactionContextBuilder;

/// Tests that the metadata extension can store and retrieve name and URI via MASM.
#[tokio::test]
async fn metadata_extension_get_from_masm() -> anyhow::Result<()> {
    // Create test values
    let name_value = Word::from([1u32, 2, 3, 4]);
    let uri_value = Word::from([5u32, 6, 7, 8]);

    // Create account with metadata extension
    let extension = MetadataExtension::new().with_name(name_value).with_uri(uri_value);

    let account = AccountBuilder::new([1u8; 32])
        .with_auth_component(NoAuth)
        .with_component(extension)
        .build()?;

    // MASM script to read metadata and verify values
    // Use `call.` to invoke account procedures with the full path
    let tx_script = format!(
        r#"
        begin
            # Get name and verify
            push.{key_name}
            call.::miden::standards::metadata::extension::get
            # => [NAME]

            push.{expected_name}
            assert_eqw.err="name does not match expected"
            # => []

            # Get URI and verify
            push.{key_uri}
            call.::miden::standards::metadata::extension::get
            # => [URI]

            push.{expected_uri}
            assert_eqw.err="uri does not match expected"
            # => []
        end
        "#,
        key_name = MetadataExtension::key_name(),
        key_uri = MetadataExtension::key_uri(),
        expected_name = name_value,
        expected_uri = uri_value,
    );

    let source_manager = Arc::new(DefaultSourceManager::default());
    let tx_script =
        CodeBuilder::with_source_manager(source_manager.clone()).compile_tx_script(tx_script)?;

    let tx_context = TransactionContextBuilder::new(account)
        .tx_script(tx_script)
        .with_source_manager(source_manager)
        .build()?;

    tx_context.execute().await?;

    Ok(())
}

/// Tests that reading a non-existent key returns EMPTY_WORD.
#[tokio::test]
async fn metadata_extension_get_non_existent_key_returns_empty() -> anyhow::Result<()> {
    // Create account with empty metadata extension
    let extension = MetadataExtension::new();

    let account = AccountBuilder::new([1u8; 32])
        .with_auth_component(NoAuth)
        .with_component(extension)
        .build()?;

    // MASM script to read non-existent key and verify it's empty
    let tx_script = format!(
        r#"
        begin
            # Get name (not set) and verify it's empty
            push.{key_name}
            call.::miden::standards::metadata::extension::get
            # => [VALUE]

            # Empty word is [0, 0, 0, 0]
            padw
            assert_eqw.err="non-existent key should return empty word"
            # => []
        end
        "#,
        key_name = MetadataExtension::key_name(),
    );

    let source_manager = Arc::new(DefaultSourceManager::default());
    let tx_script =
        CodeBuilder::with_source_manager(source_manager.clone()).compile_tx_script(tx_script)?;

    let tx_context = TransactionContextBuilder::new(account)
        .tx_script(tx_script)
        .with_source_manager(source_manager)
        .build()?;

    tx_context.execute().await?;

    Ok(())
}

/// Tests that custom keys work with the metadata extension.
#[tokio::test]
async fn metadata_extension_custom_key_from_masm() -> anyhow::Result<()> {
    // Create a custom key and value
    let custom_key = Word::from([100u32, 200, 300, 400]);
    let custom_value = Word::from([10u32, 20, 30, 40]);

    // Create account with custom metadata entry
    let extension = MetadataExtension::new().with_entry(custom_key, custom_value);

    let account = AccountBuilder::new([1u8; 32])
        .with_auth_component(NoAuth)
        .with_component(extension)
        .build()?;

    // MASM script to read custom key and verify value
    let tx_script = format!(
        r#"
        begin
            # Get custom key and verify
            push.{custom_key}
            call.::miden::standards::metadata::extension::get
            # => [VALUE]

            push.{expected_value}
            assert_eqw.err="custom key value does not match expected"
            # => []
        end
        "#,
        custom_key = custom_key,
        expected_value = custom_value,
    );

    let source_manager = Arc::new(DefaultSourceManager::default());
    let tx_script =
        CodeBuilder::with_source_manager(source_manager.clone()).compile_tx_script(tx_script)?;

    let tx_context = TransactionContextBuilder::new(account)
        .tx_script(tx_script)
        .with_source_manager(source_manager)
        .build()?;

    tx_context.execute().await?;

    Ok(())
}

/// Tests that the metadata extension works alongside a fungible faucet.
/// This demonstrates the composable nature of account components - both
/// components' storage slots are accessible from Rust.
#[test]
fn metadata_extension_with_faucet_storage() {
    use miden_protocol::Felt;
    use miden_protocol::account::AccountStorageMode;
    use miden_standards::account::faucets::BasicFungibleFaucet;

    // Create metadata values (simple test values)
    let name_value = Word::from([111u32, 222, 333, 444]);
    let uri_value = Word::from([555u32, 666, 777, 888]);

    // Create faucet with metadata extension
    let faucet = BasicFungibleFaucet::new(
        "TST".try_into().unwrap(),
        8,                    // decimals
        Felt::new(1_000_000), // max_supply
    )
    .unwrap();

    let extension = MetadataExtension::new().with_name(name_value).with_uri(uri_value);

    let account = AccountBuilder::new([1u8; 32])
        .account_type(miden_protocol::account::AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(NoAuth)
        .with_component(faucet)
        .with_component(extension)
        .build()
        .unwrap();

    // Verify faucet metadata is intact
    let faucet_metadata = account.storage().get_item(BasicFungibleFaucet::metadata_slot()).unwrap();
    assert_eq!(faucet_metadata[0], Felt::new(1_000_000)); // max_supply
    assert_eq!(faucet_metadata[1], Felt::new(8)); // decimals

    // Verify extension metadata via StorageMap
    let name_from_storage = account
        .storage()
        .get_map_item(MetadataExtension::slot(), MetadataExtension::key_name())
        .unwrap();
    let uri_from_storage = account
        .storage()
        .get_map_item(MetadataExtension::slot(), MetadataExtension::key_uri())
        .unwrap();

    assert_eq!(name_from_storage, name_value);
    assert_eq!(uri_from_storage, uri_value);
}

/// Tests that BasicFungibleFaucet with integrated name/uri works correctly.
/// This uses the faucet's built-in with_name() and with_uri() methods.
#[test]
fn faucet_with_integrated_metadata() {
    use miden_protocol::Felt;
    use miden_protocol::account::AccountStorageMode;
    use miden_standards::account::faucets::BasicFungibleFaucet;

    // Create metadata values
    let name_value = Word::from([11u32, 22, 33, 44]);
    let uri_value = Word::from([55u32, 66, 77, 88]);

    // Create faucet with integrated name and uri
    let faucet = BasicFungibleFaucet::new(
        "INT".try_into().unwrap(),
        6,                  // decimals
        Felt::new(500_000), // max_supply
    )
    .unwrap()
    .with_name(name_value)
    .with_uri(uri_value);

    // Verify the getters work
    assert_eq!(faucet.name(), Some(name_value));
    assert_eq!(faucet.uri(), Some(uri_value));

    let account = AccountBuilder::new([2u8; 32])
        .account_type(miden_protocol::account::AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(NoAuth)
        .with_component(faucet)
        .build()
        .unwrap();

    // Verify faucet metadata is intact
    let faucet_metadata = account.storage().get_item(BasicFungibleFaucet::metadata_slot()).unwrap();
    assert_eq!(faucet_metadata[0], Felt::new(500_000)); // max_supply
    assert_eq!(faucet_metadata[1], Felt::new(6)); // decimals

    // Verify extension metadata via StorageMap
    let name_from_storage = account
        .storage()
        .get_map_item(BasicFungibleFaucet::extension_slot(), MetadataExtension::key_name())
        .unwrap();
    let uri_from_storage = account
        .storage()
        .get_map_item(BasicFungibleFaucet::extension_slot(), MetadataExtension::key_uri())
        .unwrap();

    assert_eq!(name_from_storage, name_value);
    assert_eq!(uri_from_storage, uri_value);

    // Verify the faucet can be recovered from the account
    let recovered_faucet = BasicFungibleFaucet::try_from(&account).unwrap();
    assert_eq!(recovered_faucet.name(), Some(name_value));
    assert_eq!(recovered_faucet.uri(), Some(uri_value));
    assert_eq!(recovered_faucet.max_supply(), Felt::new(500_000));
    assert_eq!(recovered_faucet.decimals(), 6);
}

/// Tests that BasicFungibleFaucet metadata can be read from MASM using the faucet's get procedure.
#[tokio::test]
async fn faucet_metadata_readable_from_masm() -> anyhow::Result<()> {
    use miden_protocol::Felt;
    use miden_protocol::account::AccountStorageMode;
    use miden_standards::account::faucets::BasicFungibleFaucet;

    // Create metadata values
    let name_value = Word::from([100u32, 200, 300, 400]);
    let uri_value = Word::from([500u32, 600, 700, 800]);

    // Create faucet with integrated name and uri
    let faucet = BasicFungibleFaucet::new(
        "MAS".try_into().unwrap(),
        10,                 // decimals
        Felt::new(999_999), // max_supply
    )
    .unwrap()
    .with_name(name_value)
    .with_uri(uri_value);

    let account = AccountBuilder::new([3u8; 32])
        .account_type(miden_protocol::account::AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(NoAuth)
        .with_component(faucet)
        .build()?;

    // MASM script to read metadata via the faucet's get procedure
    let tx_script = format!(
        r#"
        begin
            # Get name via faucet's get procedure and verify
            push.{key_name}
            call.::miden::standards::metadata::extension::get
            # => [NAME]

            push.{expected_name}
            assert_eqw.err="faucet name does not match expected"
            # => []

            # Get URI via faucet's get procedure and verify
            push.{key_uri}
            call.::miden::standards::metadata::extension::get
            # => [URI]

            push.{expected_uri}
            assert_eqw.err="faucet uri does not match expected"
            # => []
        end
        "#,
        key_name = MetadataExtension::key_name(),
        key_uri = MetadataExtension::key_uri(),
        expected_name = name_value,
        expected_uri = uri_value,
    );

    let source_manager = Arc::new(DefaultSourceManager::default());
    let tx_script =
        CodeBuilder::with_source_manager(source_manager.clone()).compile_tx_script(tx_script)?;

    let tx_context = TransactionContextBuilder::new(account)
        .tx_script(tx_script)
        .with_source_manager(source_manager)
        .build()?;

    tx_context.execute().await?;

    Ok(())
}
