use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, BankMsg, Coin, ContractResult, CosmosMsg, Decimal, Reply, Response, SubMsg, SubMsgExecutionResponse, Uint128, WasmMsg, from_binary, to_binary};
use seesaw::vamm::{InstantiateMsg, ExecuteMsg, QueryMsg, StateResponse, Funding, WhoPays};
use seesaw::bank::{Direction };

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::contract::{instantiate, execute, query};
use crate::error::ContractError;
use crate::state::{MarketSnapshots, SnapshotItem};
use crate::testing::mock_querier::mock_dependencies;

#[test]
fn swap_in() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_token_balances(&[
        (
            &"liquidity0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::zero())],
        )
    ]);

    let info = mock_info("owner", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
        bank_addr: "bank0000".to_string(),
        init_quote_reserve: Uint128::from(1_000_000u128),
        init_base_reserve: Uint128::from(1_000u128)
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::SwapIn { direction: Direction::LONG, quote_asset_amount: Uint256::from(1000u128) };

    // check mint ib token
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::State { }).unwrap();
    let state: StateResponse = from_binary(&res).unwrap();

    assert_eq!(state.quote_asset_reserve, Uint256::from(1001000u128));
    assert_eq!(state.base_asset_reserve, Uint256::from(999u128));

    let res = query(deps.as_ref(), mock_env(), QueryMsg::MarketSnapshots { }).unwrap();
    let snapshots: MarketSnapshots = from_binary(&res).unwrap();

    // TEST SNAPSHOTS
    assert_eq!(snapshots, snapshots);
}

#[test]
fn swap_out() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_token_balances(&[
        (
            &"liquidity0000".to_string(),
            &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::zero())],
        )
    ]);

    let info = mock_info("owner", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
        bank_addr: "bank0000".to_string(),
        init_quote_reserve: Uint128::from(1_000_000u128),
        init_base_reserve: Uint128::from(1_000u128)
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::SwapOut { direction: Direction::LONG, base_asset_amount: Uint256::from(10u128) };

    // check mint ib token
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::State { }).unwrap();
    let state: StateResponse = from_binary(&res).unwrap();

    assert_eq!(state.quote_asset_reserve, Uint256::from(990099u128));
    assert_eq!(state.base_asset_reserve, Uint256::from(1010u128));
}

#[test]
fn funding_rates_access_control() {
    // Instantiate Contract
    let mut deps = mock_dependencies(&[]);

    let info = mock_info("creator", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
        bank_addr: "bank0000".to_string(),
        init_quote_reserve: Uint128::from(1_000_000u128),
        init_base_reserve: Uint128::from(1_000u128)
    };
    
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // Check access control
    let info = mock_info("random_person", &vec![]);

    let msg = ExecuteMsg::SettleFunding { };

    execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
}

#[test]
fn funding_rates() {
    let mut deps = mock_dependencies(&[]);

    let info = mock_info("creator", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
        bank_addr: "bank0000".to_string(),
        init_quote_reserve: Uint128::from(1_100_000u128),
        init_base_reserve: Uint128::from(1_000u128)
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("bank0000", &vec![]);
    
    let msg = ExecuteMsg::SettleFunding { };

    // check mint ib token
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::State { }).unwrap();
    let state: StateResponse = from_binary(&res).unwrap();

    let premium_fraction: Decimal256 = Decimal256::from_ratio((Uint256::from(1100u128) - Uint256::from(1000u128)),Uint256::from(3u128));
    let funding_rate = premium_fraction / Decimal256::from_uint256(1000u128);

    let funding: Funding = Funding {
        amount: funding_rate,
        who_pays: WhoPays::LONG
    };
    
    let funding_cumulative: Decimal256 = Decimal256::from_uint256(1_000_000_000u128) + premium_fraction;
    assert_eq!(state.funding_fee, funding);
    assert_eq!(state.funding_premium_cumulative, funding_cumulative);
}


// #[test]
// fn convert() {
//     let x = Uint256::from(200u128);
//     let y = uint256_to_u128(&x);

//     assert_eq!(y, 200u128)
// }




// pub fn u256_to_u128_second_half(a: &U256) -> u128 {
//     let a0 = a.0[2] as u128;
//     let a1 = (a.0[3] as u128) << 64;
//     return a0 + a1;
// }

// pub fn u256_to_u128(a: &Uint256) -> u128 {
//     let a0 = a.0[0] as u128;
//     let a1 = (a.0[1] as u128) << 64;
//     return a0 + a1;
// }

// pub fn uint256_to_u128_second_half(a: &Uint256) -> u128 {
//     return u256_to_u128_second_half(&a.0);
// }

// pub fn uint256_to_u128(a: &Uint256) -> u128 {
//     return u256_to_u128(&a.0);
// }