//! Integration tests for the Metadata Extension component.

extern crate alloc;

use alloc::sync::Arc;

use miden_protocol::Word;
use miden_protocol::account::AccountBuilder;
use miden_protocol::assembly::DefaultSourceManager;
use miden_standards::account::auth::NoAuth;
use miden_standards::account::metadata::Info;
use miden_standards::code_builder::CodeBuilder;
use miden_testing::TransactionContextBuilder;

/// Tests that the metadata extension can store and retrieve name via MASM.
#[tokio::test]
async fn metadata_info_get_name_from_masm() -> anyhow::Result<()> {
    let name = [Word::from([1u32, 2, 3, 4]), Word::from([5u32, 6, 7, 8])];

    let extension = Info::new().with_name(name);

    let account = AccountBuilder::new([1u8; 32])
        .with_auth_component(NoAuth)
        .with_component(extension)
        .build()?;

    // MASM script to read name and verify values
    let tx_script = format!(
        r#"
        begin
            # Get name (returns [NAME_CHUNK_0, NAME_CHUNK_1])
            call.::miden::standards::metadata::info::get_name
            # => [NAME_CHUNK_0, NAME_CHUNK_1]

            # Verify chunk 0 (on top)
            push.{expected_name_0}
            assert_eqw.err="name chunk 0 does not match"
            # => [NAME_CHUNK_1]

            # Verify chunk 1
            push.{expected_name_1}
            assert_eqw.err="name chunk 1 does not match"
            # => []
        end
        "#,
        expected_name_0 = name[0],
        expected_name_1 = name[1],
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

/// Tests that reading zero-valued name returns empty words.
#[tokio::test]
async fn metadata_info_get_name_zeros_returns_empty() -> anyhow::Result<()> {
    // Create extension with zero-valued name (slots exist, but contain zeros)
    let name = [Word::default(), Word::default()];
    let extension = Info::new().with_name(name);

    let account = AccountBuilder::new([1u8; 32])
        .with_auth_component(NoAuth)
        .with_component(extension)
        .build()?;

    let tx_script = r#"
        begin
            call.::miden::standards::metadata::info::get_name
            # => [NAME_CHUNK_0, NAME_CHUNK_1]

            padw assert_eqw.err="name chunk 0 should be empty"
            padw assert_eqw.err="name chunk 1 should be empty"
        end
        "#
    .to_string();

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

/// Tests that the full content URI (6 Words) can be read via MASM using get_content_uri.
#[tokio::test]
async fn metadata_info_get_content_uri_from_masm() -> anyhow::Result<()> {
    let content_uri = [
        Word::from([10u32, 11, 12, 13]),
        Word::from([14u32, 15, 16, 17]),
        Word::from([18u32, 19, 20, 21]),
        Word::from([22u32, 23, 24, 25]),
        Word::from([26u32, 27, 28, 29]),
        Word::from([30u32, 31, 32, 33]),
    ];

    let extension = Info::new().with_content_uri(content_uri);

    let account = AccountBuilder::new([1u8; 32])
        .with_auth_component(NoAuth)
        .with_component(extension)
        .build()?;

    // Test get_content_uri returns 6 words; verify the top word is CONTENT_URI_0, then drop the
    // rest.
    let tx_script = format!(
        r#"
        begin
            call.::miden::standards::metadata::info::get_content_uri
            # => [CONTENT_URI_0, CONTENT_URI_1, CONTENT_URI_2, CONTENT_URI_3, CONTENT_URI_4, CONTENT_URI_5]

            push.{expected_0}
            assert_eqw.err="content_uri_0 does not match"
            dropw dropw dropw dropw dropw
            # All 6 words returned; first word matches
        end
        "#,
        expected_0 = content_uri[0],
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
#[test]
fn metadata_info_with_faucet_storage() {
    use miden_protocol::Felt;
    use miden_protocol::account::AccountStorageMode;
    use miden_standards::account::faucets::BasicFungibleFaucet;

    let name = [Word::from([111u32, 222, 333, 444]), Word::from([555u32, 666, 777, 888])];
    let content_uri = [
        Word::from([10u32, 20, 30, 40]),
        Word::from([50u32, 60, 70, 80]),
        Word::from([90u32, 100, 110, 120]),
        Word::from([130u32, 140, 150, 160]),
        Word::from([170u32, 180, 190, 200]),
        Word::from([210u32, 220, 230, 240]),
    ];

    let faucet = BasicFungibleFaucet::new(
        "TST".try_into().unwrap(),
        8,                    // decimals
        Felt::new(1_000_000), // max_supply
    )
    .unwrap();

    let extension = Info::new().with_name(name).with_content_uri(content_uri);

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

    // Verify name chunks via value slots
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

/// Tests that BasicFungibleFaucet with integrated name/content_uri works correctly.
#[test]
fn faucet_with_integrated_metadata() {
    use miden_protocol::Felt;
    use miden_protocol::account::AccountStorageMode;
    use miden_standards::account::faucets::BasicFungibleFaucet;

    let name = [Word::from([11u32, 22, 33, 44]), Word::from([55u32, 66, 77, 88])];
    let content_uri = [
        Word::from([1u32, 2, 3, 4]),
        Word::from([5u32, 6, 7, 8]),
        Word::from([9u32, 10, 11, 12]),
        Word::from([13u32, 14, 15, 16]),
        Word::from([17u32, 18, 19, 20]),
        Word::from([21u32, 22, 23, 24]),
    ];

    let faucet = BasicFungibleFaucet::new(
        "INT".try_into().unwrap(),
        6,                  // decimals
        Felt::new(500_000), // max_supply
    )
    .unwrap()
    .with_name(name)
    .with_content_uri(content_uri);

    // Verify the getters work
    assert_eq!(faucet.name(), Some(name));
    assert_eq!(faucet.content_uri(), Some(content_uri));

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

    // Verify name chunks via value slots
    let name_0 = account.storage().get_item(Info::name_chunk_0_slot()).unwrap();
    let name_1 = account.storage().get_item(Info::name_chunk_1_slot()).unwrap();
    assert_eq!(name_0, name[0]);
    assert_eq!(name_1, name[1]);

    // Verify content URI chunks
    for (i, expected) in content_uri.iter().enumerate() {
        let chunk = account.storage().get_item(Info::content_uri_slot(i)).unwrap();
        assert_eq!(chunk, *expected);
    }

    // Verify the faucet can be recovered from the account
    let recovered_faucet = BasicFungibleFaucet::try_from(&account).unwrap();
    assert_eq!(recovered_faucet.name(), Some(name));
    assert_eq!(recovered_faucet.content_uri(), Some(content_uri));
    assert_eq!(recovered_faucet.max_supply(), Felt::new(500_000));
    assert_eq!(recovered_faucet.decimals(), 6);
}

/// Tests that BasicFungibleFaucet metadata can be read from MASM using the faucet's procedures.
#[tokio::test]
async fn faucet_metadata_readable_from_masm() -> anyhow::Result<()> {
    use miden_protocol::Felt;
    use miden_protocol::account::AccountStorageMode;
    use miden_standards::account::faucets::BasicFungibleFaucet;

    let name = [Word::from([100u32, 200, 300, 400]), Word::from([500u32, 600, 700, 800])];
    let content_uri = [
        Word::from([1u32, 2, 3, 4]),
        Word::from([5u32, 6, 7, 8]),
        Word::from([9u32, 10, 11, 12]),
        Word::from([13u32, 14, 15, 16]),
        Word::from([17u32, 18, 19, 20]),
        Word::from([21u32, 22, 23, 24]),
    ];

    let faucet = BasicFungibleFaucet::new(
        "MAS".try_into().unwrap(),
        10,                 // decimals
        Felt::new(999_999), // max_supply
    )
    .unwrap()
    .with_name(name)
    .with_content_uri(content_uri);

    let account = AccountBuilder::new([3u8; 32])
        .account_type(miden_protocol::account::AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(NoAuth)
        .with_component(faucet)
        .build()?;

    // MASM script to read name and full content URI via the extension procedures and verify
    let tx_script = format!(
        r#"
        begin
            # Get name and verify
            call.::miden::standards::metadata::info::get_name
            # => [NAME_CHUNK_0, NAME_CHUNK_1]

            push.{expected_name_0}
            assert_eqw.err="faucet name chunk 0 does not match"

            push.{expected_name_1}
            assert_eqw.err="faucet name chunk 1 does not match"

            # Get content URI (6 words) and verify first chunk (CONTENT_URI_0 on top)
            call.::miden::standards::metadata::info::get_content_uri
            push.{expected_uri_0}
            assert_eqw.err="faucet content_uri_0 does not match"
            dropw dropw dropw dropw dropw
        end
        "#,
        expected_name_0 = name[0],
        expected_name_1 = name[1],
        expected_uri_0 = content_uri[0],
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
