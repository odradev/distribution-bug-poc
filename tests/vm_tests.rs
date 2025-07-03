use casper_engine_test_support::genesis_config_builder::GenesisConfigBuilder;
use casper_engine_test_support::{ChainspecConfig, ExecuteRequestBuilder, LmdbWasmTestBuilder, CHAINSPEC_SYMLINK, DEFAULT_ACCOUNT_INITIAL_BALANCE, DEFAULT_AUCTION_DELAY, DEFAULT_CHAINSPEC_REGISTRY, DEFAULT_GENESIS_CONFIG_HASH, DEFAULT_GENESIS_TIMESTAMP_MILLIS, DEFAULT_LOCKED_FUNDS_PERIOD_MILLIS, DEFAULT_PROTOCOL_VERSION, DEFAULT_ROUND_SEIGNIORAGE_RATE, DEFAULT_SYSTEM_CONFIG, DEFAULT_UNBONDING_DELAY, DEFAULT_VALIDATOR_SLOTS, DEFAULT_WASM_CONFIG};
use casper_storage::data_access_layer::GenesisRequest;
use casper_types::system::auction::{BidAddr, ARG_AMOUNT, ARG_DELEGATOR_PURSE, ARG_VALIDATOR, METHOD_ADD_BID, METHOD_DELEGATE, METHOD_UNDELEGATE};
use casper_types::{runtime_args, GenesisAccount, GenesisConfig, GenesisValidator, Key, Motes, ProtocolVersion, PublicKey, RuntimeArgs, SecretKey, U512};
use std::collections::BTreeMap;
use std::path::PathBuf;
use casper_types::system::auction;

struct TestEnv {
    pub context: LmdbWasmTestBuilder,
    pub account: GenesisAccount,
    pub validator: GenesisAccount,
}

impl TestEnv {
    fn new_instance() -> Self {
        let genesis_accounts = vec![Self::genesis_account(), Self::validator_account()];
        let genesis_config = Self::genesis_config(genesis_accounts.clone());

        let mut genesis_request = GenesisRequest::new(
            DEFAULT_GENESIS_CONFIG_HASH,
            ProtocolVersion::V2_0_0,
            genesis_config,
            DEFAULT_CHAINSPEC_REGISTRY.clone(),
        );
        genesis_request.set_enable_entity(false);

        let chainspec_path = PathBuf::from("resources/chainspec.toml");
        let c = ChainspecConfig::create_genesis_request_from_local_chainspec(genesis_accounts.clone(), DEFAULT_PROTOCOL_VERSION);
        let mut builder = LmdbWasmTestBuilder::default();

        builder.run_genesis(c.unwrap()).commit();
        builder.advance_eras_by(10);


        let bid_request = ExecuteRequestBuilder::contract_call_by_hash(
            Self::validator_account().account_hash(),
            builder.get_auction_contract_hash(),
            METHOD_ADD_BID,
            runtime_args! {
            auction::ARG_PUBLIC_KEY => Self::validator_account().public_key(),
            auction::ARG_AMOUNT => U512::from(1000_000_000_000u64),
            auction::ARG_DELEGATION_RATE => 0u8,
                auction::ARG_MINIMUM_DELEGATION_AMOUNT => 400_000_000_000u64,
                }
        )
            .build();

        builder.exec(bid_request).commit().expect_success();

        let query = builder.query(None, Key::BidAddr(BidAddr::Validator(Self::validator_account().account_hash())), &[]);
        dbg!(&query);

        builder.advance_eras_by(10);



        Self {
            context: builder,
            account: genesis_accounts[0].clone(),
            validator: genesis_accounts[1].clone(),
        }
    }

    fn genesis_config(genesis_accounts: Vec<GenesisAccount>) -> GenesisConfig {
        GenesisConfigBuilder::default()
            .with_accounts(genesis_accounts)
            .with_wasm_config(*DEFAULT_WASM_CONFIG)
            .with_system_config(*DEFAULT_SYSTEM_CONFIG)
            .with_validator_slots(DEFAULT_VALIDATOR_SLOTS)
            .with_auction_delay(DEFAULT_AUCTION_DELAY)
            .with_locked_funds_period_millis(DEFAULT_LOCKED_FUNDS_PERIOD_MILLIS)
            .with_round_seigniorage_rate(DEFAULT_ROUND_SEIGNIORAGE_RATE)
            .with_unbonding_delay(DEFAULT_UNBONDING_DELAY)
            .with_genesis_timestamp_millis(DEFAULT_GENESIS_TIMESTAMP_MILLIS)
            .build()
    }

    fn genesis_account() -> GenesisAccount {
        let sk = SecretKey::ed25519_from_bytes([7u8; 32]).unwrap();
        GenesisAccount::account(
            PublicKey::from(&sk),
            Motes::new(DEFAULT_ACCOUNT_INITIAL_BALANCE),
            None,
        )
    }

    fn validator_account() -> GenesisAccount {
        let sk = SecretKey::ed25519_from_bytes([8u8; 32]).unwrap();
        GenesisAccount::account(
            PublicKey::from(&sk),
            Motes::new(DEFAULT_ACCOUNT_INITIAL_BALANCE),
            Some(GenesisValidator::new(
                Motes::new(DEFAULT_ACCOUNT_INITIAL_BALANCE),
                0,
            )),
        )
    }
}

#[test]
fn test() {
    let mut test_env = TestEnv::new_instance();
    let account_purse = test_env
        .context
        .get_account(test_env.account.account_hash())
        .unwrap()
        .main_purse();
    let balance_before = test_env.context.get_purse_balance(account_purse);

    let stake_amount = U512::from(1000_000_000_000u64);

    // account delegates 1000 CSPR
    let mut args = RuntimeArgs::new();
    args.insert(ARG_DELEGATOR_PURSE, account_purse).unwrap();
    args.insert(ARG_VALIDATOR, test_env.validator.public_key())
        .unwrap();
    args.insert(ARG_AMOUNT, stake_amount)
        .unwrap();
    let delegate_request = ExecuteRequestBuilder::contract_call_by_hash(
        test_env.account.account_hash(),
        test_env.context.get_auction_contract_hash(),
        METHOD_DELEGATE,
        args,
    )
    .build();

    test_env
        .context
        .exec(delegate_request)
        .commit()
        .expect_success();

    let balance_after_delegate = test_env.context.get_purse_balance(account_purse);

    assert_eq!(
        balance_before,
        balance_after_delegate + stake_amount
    );

    // Some time passes
    test_env.context.advance_eras_by_default_auction_delay();
    // What is blocktime here?
    test_env
        .context
        .distribute(None, ProtocolVersion::V2_0_0, BTreeMap::new(), 0);


    let bid_request = ExecuteRequestBuilder::contract_call_by_hash(
        test_env.validator.account_hash(),
        test_env.context.get_auction_contract_hash(),
        METHOD_ADD_BID,
        runtime_args! {
            auction::ARG_PUBLIC_KEY => test_env.validator.public_key(),
            auction::ARG_AMOUNT => U512::from(1000_000_000_000u64),
            auction::ARG_DELEGATION_RATE => 0u8,
                auction::ARG_MINIMUM_DELEGATION_AMOUNT => 1100_000_000_000u64,
                }
    )
        .build();

    test_env.context.exec(bid_request).commit().expect_success();

    let auction_delay = test_env.context.get_auction_delay();
    let unbonding_delay = test_env.context.get_unbonding_delay();


    let bid = test_env
        .context
        .get_bids()
        .into_iter()
        .find(|bid| {
            bid.validator_public_key() == test_env.validator.public_key() && bid.is_delegator()
        });


    test_env.context.advance_eras_by(unbonding_delay + auction_delay);

    dbg!(test_env.context.get_unbonds());

    assert_eq!(bid, None, "Delegator został wyrzucony od razu");


    dbg!(&bid);
    panic!();


    // Undelegate 750 CSPR
    let mut args = RuntimeArgs::new();
    args.insert(ARG_DELEGATOR_PURSE, account_purse).unwrap();
    args.insert(ARG_VALIDATOR, test_env.validator.public_key())
        .unwrap();
    args.insert(ARG_AMOUNT, U512::from(750_000_000_000u64))
        .unwrap();
    let undelegate_request = ExecuteRequestBuilder::contract_call_by_hash(
        test_env.account.account_hash(),
        test_env.context.get_auction_contract_hash(),
        METHOD_UNDELEGATE,
        args,
    )
    .build();

    test_env
        .context
        .exec(undelegate_request)
        .commit()
        .expect_success();

    // Some time passes again
    test_env.context.advance_eras_by(10);
    test_env
        .context
        .distribute(None, ProtocolVersion::V2_0_0, BTreeMap::new(), 0);

    let balance_after_undelegate = test_env.context.get_purse_balance(account_purse);

    // User receives 750 CSPR back
    assert_eq!(
        balance_after_undelegate,
        balance_after_delegate + U512::from(1_000_000_000_000u64)
    );

    let bid = test_env
        .context
        .get_bids()
        .into_iter()
        .find(|bid| {
            bid.validator_public_key() == test_env.validator.public_key() && bid.is_delegator()
        })
        .unwrap();

    // And 250 CSPR is still staked
    assert_eq!(bid.staked_amount().unwrap(), U512::from(250_000_000_000u64));
}
