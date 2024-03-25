use std::collections::HashMap;

use assert_matches::assert_matches;

use casper_engine_test_support::{
    DeployItemBuilder, ExecuteRequestBuilder, LmdbWasmTestBuilder, DEFAULT_ACCOUNT_ADDR,
    DEFAULT_ACCOUNT_INITIAL_BALANCE, DEFAULT_ACCOUNT_KEY, DEFAULT_GAS_PRICE, DEFAULT_PAYMENT,
    LOCAL_GENESIS_REQUEST, MINIMUM_ACCOUNT_CREATION_BALANCE,
};
use casper_execution_engine::{
    engine_state::{Error, MAX_PAYMENT},
    execution::ExecError,
};
use casper_types::{
    account::AccountHash, execution::TransformKindV2, runtime_args, system::handle_payment,
    ApiError, Gas, Key, Motes, RuntimeArgs, U512,
};

const ACCOUNT_1_ADDR: AccountHash = AccountHash::new([42u8; 32]);
const DO_NOTHING_WASM: &str = "do_nothing.wasm";
const TRANSFER_PURSE_TO_ACCOUNT_WASM: &str = "transfer_purse_to_account.wasm";
const REVERT_WASM: &str = "revert.wasm";
const ENDLESS_LOOP_WASM: &str = "endless_loop.wasm";
const ARG_AMOUNT: &str = "amount";
const ARG_TARGET: &str = "target";

#[ignore]
#[allow(unused)]
// #[test]
fn should_raise_insufficient_payment_when_caller_lacks_minimum_balance() {
    let account_1_account_hash = ACCOUNT_1_ADDR;

    let exec_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        TRANSFER_PURSE_TO_ACCOUNT_WASM,
        runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => *MAX_PAYMENT - U512::one() },
    )
        .build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder
        .run_genesis(LOCAL_GENESIS_REQUEST.clone())
        .exec(exec_request)
        .expect_success()
        .commit()
        .get_exec_result_owned(0)
        .expect("there should be a response");

    let account_1_request =
        ExecuteRequestBuilder::standard(ACCOUNT_1_ADDR, REVERT_WASM, RuntimeArgs::default())
            .build();

    let error_message = builder
        .exec(account_1_request)
        .commit()
        .get_error_message()
        .expect("there should be a response");

    assert!(
        error_message.contains("Insufficient payment"),
        "expected insufficient payment, got: {}",
        error_message
    );

    let expected_transfers_count = 0;
    let effects = &builder.get_effects()[1];

    assert_eq!(
        effects.transforms().len(),
        expected_transfers_count,
        "there should be no transforms if the account main purse has less than max payment"
    );
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_forward_payment_execution_runtime_error() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let transferred_amount = U512::from(1);

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_deploy_hash([1; 32])
            .with_payment_code(REVERT_WASM, RuntimeArgs::default())
            .with_session_code(
                TRANSFER_PURSE_TO_ACCOUNT_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => transferred_amount },
            )
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder.run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let proposer_reward_starting_balance = builder.get_proposer_purse_balance();

    builder.exec(exec_request).commit();

    let transaction_fee = builder.get_proposer_purse_balance() - proposer_reward_starting_balance;
    let initial_balance: U512 = U512::from(DEFAULT_ACCOUNT_INITIAL_BALANCE);
    let expected_reward_balance = *MAX_PAYMENT;

    let modified_balance = builder.get_purse_balance(
        builder
            .get_entity_by_account_hash(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account")
            .main_purse(),
    );

    assert_eq!(
        modified_balance,
        initial_balance - expected_reward_balance,
        "modified balance is incorrect"
    );

    assert_eq!(
        transaction_fee, expected_reward_balance,
        "transaction fee is incorrect"
    );

    assert_eq!(
        initial_balance,
        (modified_balance + transaction_fee),
        "no net resources should be gained or lost post-distribution"
    );

    let exec_result = builder
        .get_exec_result_owned(0)
        .expect("there should be a response");

    let error = exec_result.error().expect("should have error");
    assert_matches!(error, Error::Exec(ExecError::Revert(ApiError::User(100))));
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_forward_payment_execution_gas_limit_error() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let transferred_amount = U512::from(1);

    let mut builder = LmdbWasmTestBuilder::default();

    builder.run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_deploy_hash([1; 32])
            .with_payment_code(ENDLESS_LOOP_WASM, RuntimeArgs::default())
            .with_session_code(
                TRANSFER_PURSE_TO_ACCOUNT_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => transferred_amount },
            )
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let proposer_reward_starting_balance = builder.get_proposer_purse_balance();

    builder.exec(exec_request).commit();

    let transaction_fee = builder.get_proposer_purse_balance() - proposer_reward_starting_balance;
    let initial_balance: U512 = U512::from(DEFAULT_ACCOUNT_INITIAL_BALANCE);
    let expected_reward_balance = *MAX_PAYMENT;

    let modified_balance = builder.get_purse_balance(
        builder
            .get_entity_by_account_hash(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account")
            .main_purse(),
    );

    assert_eq!(
        modified_balance,
        initial_balance - expected_reward_balance,
        "modified balance is incorrect"
    );

    assert_eq!(
        transaction_fee, expected_reward_balance,
        "transaction fee is incorrect"
    );

    assert_eq!(
        initial_balance,
        (modified_balance + transaction_fee),
        "no net resources should be gained or lost post-distribution"
    );

    let exec_result = builder
        .get_exec_result_owned(0)
        .expect("there should be a response");

    let error = exec_result.error().expect("should have error");
    assert_matches!(error, Error::Exec(ExecError::GasLimit));
    let payment_gas_limit = Gas::from_motes(Motes::new(*MAX_PAYMENT), DEFAULT_GAS_PRICE)
        .expect("should convert to gas");
    assert_eq!(
        exec_result.consumed(),
        payment_gas_limit,
        "cost should equal gas limit"
    );
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_run_out_of_gas_when_session_code_exceeds_gas_limit() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let payment_purse_amount = *DEFAULT_PAYMENT;
    let transferred_amount = 1;

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_deploy_hash([1; 32])
            .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
            .with_session_code(
                ENDLESS_LOOP_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => U512::from(transferred_amount) },
            )
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder
        .run_genesis(LOCAL_GENESIS_REQUEST.clone())
        .exec(exec_request)
        .commit();

    let exec_result = builder
        .get_exec_result_owned(0)
        .expect("there should be a response");

    let error = exec_result.error().expect("should have error");
    assert_matches!(error, Error::Exec(ExecError::GasLimit));
    let session_gas_limit = Gas::from_motes(Motes::new(payment_purse_amount), DEFAULT_GAS_PRICE)
        .expect("should convert to gas");
    assert_eq!(
        exec_result.consumed(),
        session_gas_limit,
        "cost should equal gas limit"
    );
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_correctly_charge_when_session_code_runs_out_of_gas() {
    let payment_purse_amount = *DEFAULT_PAYMENT;

    let deploy_item = DeployItemBuilder::new()
        .with_address(*DEFAULT_ACCOUNT_ADDR)
        .with_deploy_hash([1; 32])
        .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
        .with_session_code(ENDLESS_LOOP_WASM, RuntimeArgs::default())
        .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
        .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder
        .run_genesis(LOCAL_GENESIS_REQUEST.clone())
        .exec(exec_request)
        .commit();

    let default_account = builder
        .get_entity_by_account_hash(*DEFAULT_ACCOUNT_ADDR)
        .expect("should get genesis account");
    let modified_balance: U512 = builder.get_purse_balance(default_account.main_purse());
    let initial_balance: U512 = U512::from(DEFAULT_ACCOUNT_INITIAL_BALANCE);

    assert_ne!(
        modified_balance, initial_balance,
        "balance should be less than initial balance"
    );

    let exec_result = builder
        .get_exec_result_owned(0)
        .expect("there should be a response");

    let gas = exec_result.consumed();
    let motes = Motes::from_gas(gas, DEFAULT_GAS_PRICE).expect("should have motes");

    let tally = motes.value() + modified_balance;

    assert_eq!(
        initial_balance, tally,
        "no net resources should be gained or lost post-distribution"
    );

    let error = exec_result.error().expect("should have error");
    assert_matches!(error, Error::Exec(ExecError::GasLimit));
    let session_gas_limit = Gas::from_motes(Motes::new(payment_purse_amount), DEFAULT_GAS_PRICE)
        .expect("should convert to gas");
    assert_eq!(
        exec_result.consumed(),
        session_gas_limit,
        "cost should equal gas limit"
    );
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_correctly_charge_when_session_code_fails() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let payment_purse_amount = *DEFAULT_PAYMENT;
    let transferred_amount = 1;

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_deploy_hash([1; 32])
            .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
            .with_session_code(
                REVERT_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => U512::from(transferred_amount) },
            )
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder.run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let proposer_reward_starting_balance = builder.get_proposer_purse_balance();

    builder.exec(exec_request).commit();

    let default_account = builder
        .get_entity_by_account_hash(*DEFAULT_ACCOUNT_ADDR)
        .expect("should get genesis account");
    let modified_balance: U512 = builder.get_purse_balance(default_account.main_purse());
    let initial_balance: U512 = U512::from(DEFAULT_ACCOUNT_INITIAL_BALANCE);

    assert_ne!(
        modified_balance, initial_balance,
        "balance should be less than initial balance"
    );

    let transaction_fee = builder.get_proposer_purse_balance() - proposer_reward_starting_balance;
    let tally = transaction_fee + modified_balance;

    assert_eq!(
        initial_balance, tally,
        "no net resources should be gained or lost post-distribution"
    );
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_correctly_charge_when_session_code_succeeds() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let payment_purse_amount = *DEFAULT_PAYMENT;
    let transferred_amount = 1;

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_deploy_hash([1; 32])
            .with_session_code(
                TRANSFER_PURSE_TO_ACCOUNT_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => U512::from(transferred_amount) },
            )
            .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder.run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let proposer_reward_starting_balance_1 = builder.get_proposer_purse_balance();

    builder.exec(exec_request).expect_success().commit();

    let default_account = builder
        .get_entity_by_account_hash(*DEFAULT_ACCOUNT_ADDR)
        .expect("should get genesis account");
    let modified_balance: U512 = builder.get_purse_balance(default_account.main_purse());
    let initial_balance: U512 = U512::from(DEFAULT_ACCOUNT_INITIAL_BALANCE);

    assert_ne!(
        modified_balance, initial_balance,
        "balance should be less than initial balance"
    );

    let transaction_fee_1 =
        builder.get_proposer_purse_balance() - proposer_reward_starting_balance_1;

    let total = transaction_fee_1 + U512::from(transferred_amount);
    let tally = total + modified_balance;

    assert_eq!(
        initial_balance, tally,
        "no net resources should be gained or lost post-distribution"
    );
    assert_eq!(
        initial_balance, tally,
        "no net resources should be gained or lost post-distribution"
    )
}

#[ignore]
#[allow(unused)]
// #[test]
fn should_finalize_to_rewards_purse() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let payment_purse_amount = *DEFAULT_PAYMENT;
    let transferred_amount = 1;

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_session_code(
                TRANSFER_PURSE_TO_ACCOUNT_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => U512::from(transferred_amount) },
            )
            .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .with_deploy_hash([1; 32])
            .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let mut builder = LmdbWasmTestBuilder::default();

    builder.run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let proposer_reward_starting_balance = builder.get_proposer_purse_balance();

    builder.exec(exec_request).expect_success().commit();

    let modified_reward_starting_balance = builder.get_proposer_purse_balance();

    assert!(
        modified_reward_starting_balance > proposer_reward_starting_balance,
        "proposer's balance should be higher after exec"
    );
}

#[ignore]
#[allow(unused)]
// #[test]
fn independent_standard_payments_should_not_write_the_same_keys() {
    let account_1_account_hash = ACCOUNT_1_ADDR;
    let payment_purse_amount = *DEFAULT_PAYMENT;
    let transfer_amount = MINIMUM_ACCOUNT_CREATION_BALANCE;

    let mut builder = LmdbWasmTestBuilder::default();

    let deploy_item = DeployItemBuilder::new()
            .with_address(*DEFAULT_ACCOUNT_ADDR)
            .with_session_code(
                TRANSFER_PURSE_TO_ACCOUNT_WASM,
                runtime_args! { ARG_TARGET => account_1_account_hash, ARG_AMOUNT => U512::from(transfer_amount) },
            )
            .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
            .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
            .with_deploy_hash([1; 32])
            .build();

    let setup_exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    // create another account via transfer
    builder
        .run_genesis(LOCAL_GENESIS_REQUEST.clone())
        .exec(setup_exec_request)
        .expect_success()
        .commit();

    let deploy_item = DeployItemBuilder::new()
        .with_address(*DEFAULT_ACCOUNT_ADDR)
        .with_session_code(DO_NOTHING_WASM, RuntimeArgs::default())
        .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
        .with_authorization_keys(&[*DEFAULT_ACCOUNT_KEY])
        .with_deploy_hash([2; 32])
        .build();

    let exec_request_from_genesis = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let deploy_item = DeployItemBuilder::new()
        .with_address(ACCOUNT_1_ADDR)
        .with_session_code(DO_NOTHING_WASM, RuntimeArgs::default())
        .with_empty_payment_bytes(runtime_args! { ARG_AMOUNT => payment_purse_amount })
        .with_authorization_keys(&[account_1_account_hash])
        .with_deploy_hash([1; 32])
        .build();

    let exec_request_from_account_1 = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    // run two independent deploys
    builder
        .exec(exec_request_from_genesis)
        .expect_success()
        .commit()
        .exec(exec_request_from_account_1)
        .expect_success()
        .commit();

    let effects = builder.get_effects();
    let effects_from_genesis = &effects[1];
    let effects_from_account_1 = &effects[2];

    // Retrieve the payment purse.
    let payment_purse = builder
        .get_handle_payment_contract()
        .named_keys()
        .get(handle_payment::PAYMENT_PURSE_KEY)
        .unwrap()
        .into_uref()
        .unwrap();

    let transforms_from_genesis_map: HashMap<Key, TransformKindV2> = effects_from_genesis
        .transforms()
        .iter()
        .map(|transform| (*transform.key(), transform.kind().clone()))
        .collect();
    let transforms_from_account_1_map: HashMap<Key, TransformKindV2> = effects_from_account_1
        .transforms()
        .iter()
        .map(|transform| (*transform.key(), transform.kind().clone()))
        .collect();

    // Confirm the two deploys have no overlapping writes except for the payment purse balance.
    let common_write_keys = effects_from_genesis
        .transforms()
        .iter()
        .filter_map(|transform| {
            if transform.key() != &Key::Balance(payment_purse.addr())
                && matches!(
                    (
                        transforms_from_genesis_map.get(transform.key()),
                        transforms_from_account_1_map.get(transform.key()),
                    ),
                    (
                        Some(TransformKindV2::Write(_)),
                        Some(TransformKindV2::Write(_))
                    )
                )
            {
                Some(*transform.key())
            } else {
                None
            }
        });

    assert_eq!(common_write_keys.count(), 0);
}
