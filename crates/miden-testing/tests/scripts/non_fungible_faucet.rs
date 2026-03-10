extern crate alloc;

use assert_matches::assert_matches;
use miden_protocol::account::auth::{AuthScheme, PublicKeyCommitment};
use miden_protocol::account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot};
use miden_protocol::asset::TokenSymbol;
use miden_protocol::{Felt, Word};
use miden_standards::AuthMethod;
use miden_standards::account::auth::{AuthSingleSig, AuthSingleSigAcl};
use miden_standards::account::faucets::{
    BasicNonFungibleFaucet, NetworkNonFungibleFaucet, NftMetadata, NonFungibleFaucetError,
    create_basic_non_fungible_faucet,
};
use miden_standards::account::wallets::BasicWallet;

// NFT METADATA TESTS
// ================================================================================================

#[test]
fn nft_metadata_new() {
    let symbol = TokenSymbol::new("TCKT").unwrap();
    let meta = NftMetadata::new(symbol, Felt::new(10_000)).unwrap();

    assert_eq!(meta.current_supply(), 0);
    assert_eq!(meta.max_supply(), Felt::new(10_000));
    assert_eq!(meta.next_token_id(), 0);
    assert_eq!(meta.symbol(), symbol);
}

#[test]
fn nft_metadata_zero_max_supply_rejected() {
    let symbol = TokenSymbol::new("NFT").unwrap();
    let err = NftMetadata::new(symbol, Felt::new(0)).unwrap_err();
    assert!(matches!(err, NonFungibleFaucetError::MaxSupplyCannotBeZero));
}

#[test]
fn nft_metadata_to_word() {
    let symbol = TokenSymbol::new("ART").unwrap();
    let meta = NftMetadata::new(symbol, Felt::new(500)).unwrap();
    let word: Word = meta.into();

    assert_eq!(word[0], Felt::new(0)); // current_supply
    assert_eq!(word[1], Felt::new(500)); // max_supply
    assert_eq!(word[2], Felt::new(0)); // next_token_id
    assert_eq!(word[3], Felt::from(symbol)); // symbol
}

#[test]
fn nft_metadata_word_roundtrip() {
    let symbol = TokenSymbol::new("ART").unwrap();
    let original =
        NftMetadata::with_supply(symbol, Felt::new(1000), Felt::new(50), Felt::new(75)).unwrap();

    let word: Word = original.into();
    let restored = NftMetadata::try_from(word).unwrap();

    assert_eq!(restored.symbol(), symbol);
    assert_eq!(restored.max_supply(), Felt::new(1000));
    assert_eq!(restored.current_supply(), 50);
    assert_eq!(restored.next_token_id(), 75);
}

#[test]
fn nft_metadata_from_storage_slot() {
    let symbol = TokenSymbol::new("POL").unwrap();
    let original = NftMetadata::new(symbol, Felt::new(123)).unwrap();
    let slot: StorageSlot = original.into();

    let restored = NftMetadata::try_from(&slot).unwrap();

    assert_eq!(restored.symbol(), symbol);
    assert_eq!(restored.max_supply(), Felt::new(123));
    assert_eq!(restored.current_supply(), 0);
    assert_eq!(restored.next_token_id(), 0);
}

#[test]
fn nft_metadata_supply_exceeds_max() {
    let symbol = TokenSymbol::new("ART").unwrap();
    let err =
        NftMetadata::with_supply(symbol, Felt::new(100), Felt::new(101), Felt::new(101))
            .unwrap_err();

    assert!(matches!(
        err,
        NonFungibleFaucetError::SupplyExceedsMaxSupply {
            current_supply: 101,
            max_supply: 100
        }
    ));
}

// BASIC NON-FUNGIBLE FAUCET TESTS
// ================================================================================================

#[test]
fn basic_nff_faucet_contract_creation() {
    let pub_key_word = Word::new([Felt::ONE; 4]);
    let auth_method: AuthMethod = AuthMethod::SingleSig {
        approver: (pub_key_word.into(), AuthScheme::Falcon512Poseidon2),
    };

    let init_seed: [u8; 32] = [
        90, 110, 209, 94, 84, 105, 250, 242, 223, 203, 216, 124, 22, 159, 14, 132, 215, 85, 183,
        204, 149, 90, 166, 68, 100, 73, 106, 168, 125, 237, 138, 16,
    ];

    let max_supply = Felt::new(10_000);
    let token_symbol = TokenSymbol::new("TCKT").unwrap();
    let storage_mode = AccountStorageMode::Private;

    let faucet_account = create_basic_non_fungible_faucet(
        init_seed,
        token_symbol,
        max_supply,
        storage_mode,
        auth_method,
    )
    .unwrap();

    // The auth component's public key should be present.
    assert_eq!(
        faucet_account
            .storage()
            .get_item(AuthSingleSigAcl::public_key_slot())
            .unwrap(),
        pub_key_word
    );

    // Check that NFT metadata was initialized correctly.
    assert_eq!(
        faucet_account
            .storage()
            .get_item(BasicNonFungibleFaucet::metadata_slot())
            .unwrap(),
        [Felt::ZERO, Felt::new(10_000), Felt::ZERO, token_symbol.into()].into()
    );

    assert!(faucet_account.is_faucet());
    assert_eq!(faucet_account.account_type(), AccountType::NonFungibleFaucet);

    // Verify the faucet can be extracted and has correct metadata
    let faucet_component = BasicNonFungibleFaucet::try_from(faucet_account.clone()).unwrap();
    assert_eq!(faucet_component.symbol(), token_symbol);
    assert_eq!(faucet_component.max_supply(), max_supply);
    assert_eq!(faucet_component.current_supply(), 0);
    assert_eq!(faucet_component.next_token_id(), 0);
}

#[test]
fn basic_nff_faucet_create_from_account() {
    let mock_word = Word::from([0, 1, 2, 3u32]);
    let mock_public_key = PublicKeyCommitment::from(mock_word);
    let mock_seed = mock_word.as_bytes();

    let token_symbol = TokenSymbol::new("ART").expect("invalid token symbol");
    let faucet_account = AccountBuilder::new(mock_seed)
        .account_type(AccountType::NonFungibleFaucet)
        .with_component(
            BasicNonFungibleFaucet::new(token_symbol, Felt::new(500))
                .expect("failed to create a non-fungible faucet component"),
        )
        .with_auth_component(AuthSingleSig::new(
            mock_public_key,
            AuthScheme::Falcon512Poseidon2,
        ))
        .build_existing()
        .expect("failed to create faucet account");

    let basic_nff = BasicNonFungibleFaucet::try_from(faucet_account)
        .expect("basic non-fungible faucet creation failed");
    assert_eq!(basic_nff.symbol(), token_symbol);
    assert_eq!(basic_nff.max_supply(), Felt::new(500));
    assert_eq!(basic_nff.current_supply(), 0);
    assert_eq!(basic_nff.next_token_id(), 0);

    // invalid account: basic non-fungible faucet component is missing
    let invalid_faucet_account = AccountBuilder::new(mock_seed)
        .account_type(AccountType::NonFungibleFaucet)
        .with_auth_component(AuthSingleSig::new(
            mock_public_key,
            AuthScheme::Falcon512Poseidon2,
        ))
        .with_component(BasicWallet)
        .build_existing()
        .expect("failed to create faucet account");

    let err = BasicNonFungibleFaucet::try_from(invalid_faucet_account)
        .err()
        .expect("basic non-fungible faucet creation should fail");
    assert_matches!(err, NonFungibleFaucetError::MissingBasicNonFungibleFaucetInterface);
}

#[test]
fn basic_nff_get_faucet_procedures() {
    let _mint_digest = BasicNonFungibleFaucet::mint_digest();
    let _burn_digest = BasicNonFungibleFaucet::burn_digest();
}

// NETWORK NON-FUNGIBLE FAUCET TESTS
// ================================================================================================

#[test]
fn network_nff_get_faucet_procedures() {
    let _mint_digest = NetworkNonFungibleFaucet::mint_digest();
    let _burn_digest = NetworkNonFungibleFaucet::burn_digest();
}
