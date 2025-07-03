use casper_engine_test_support::{ExecuteRequestBuilder, LmdbWasmTestBuilder, DEFAULT_ACCOUNT_INITIAL_BALANCE, LOCAL_GENESIS_REQUEST};
use casper_types::system::auction::{ARG_AMOUNT, ARG_DELEGATOR_PURSE, ARG_VALIDATOR, METHOD_DELEGATE};
use casper_types::{GenesisAccount, GenesisValidator, Motes, PublicKey, RuntimeArgs, SecretKey, U512};

struct TestEnv {}

impl TestEnv {
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
    let mut genesis_request = LOCAL_GENESIS_REQUEST.clone();
    let validator = TestEnv::validator_account();
    let account = TestEnv::genesis_account();
    genesis_request.push_genesis_account(account.clone());
    genesis_request
        .push_genesis_validator(&validator.public_key(), *validator.validator().unwrap());

    let mut context = LmdbWasmTestBuilder::default();
    context
        .run_genesis(genesis_request)
        .commit()
        .advance_eras_by_default_auction_delay();

    // account delegates 1000 CSPR
    let mut args = RuntimeArgs::new();
    args.insert(ARG_DELEGATOR_PURSE, context.get_account(account.account_hash()).unwrap().main_purse()).unwrap();
    args.insert(ARG_VALIDATOR, validator.public_key()).unwrap();
    args.insert(ARG_AMOUNT, U512::from(1_000_000_000_000u64)).unwrap();
    let delegate_request = ExecuteRequestBuilder::contract_call_by_hash(
        account.account_hash(),
        context.get_auction_contract_hash(),
        METHOD_DELEGATE,
        args
    ).build();
    
    context.exec(delegate_request).commit().expect_success();
    
}
