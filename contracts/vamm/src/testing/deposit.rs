use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, BankMsg, Coin, ContractResult, CosmosMsg, Decimal, Reply, Response, SubMsg, SubMsgExecutionResponse, Uint128, WasmMsg, from_binary, to_binary};
use seesaw::vamm::{InstantiateMsg, ExecuteMsg, QueryMsg, StateResponse };
use seesaw::bank::{Direction };

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::contract::{instantiate, execute, query};
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
        init_quote_reserve: Uint256::from(1000u128),
        init_base_reserve: Uint256::from(100000u128)
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::SwapIn { direction: Direction::LONG, quote_asset_amount: Uint256::from(10u128) };

    // check mint ib token
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::State { }).unwrap();
    let state: StateResponse = from_binary(&res).unwrap();

    assert_eq!(state.quote_asset_reserve, Uint256::from(990u128));
    assert_eq!(state.base_asset_reserve, Uint256::from(101010u128));

}
