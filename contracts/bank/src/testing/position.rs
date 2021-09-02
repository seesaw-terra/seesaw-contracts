use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, BankMsg, Coin, ContractResult, CosmosMsg, Decimal, Reply, SubMsg, SubMsgExecutionResponse, Uint128, WasmMsg, from_binary, to_binary};
use seesaw::bank::{Direction, ExecuteMsg, InstantiateMsg, PositionResponse, QueryMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::contract::{instantiate, execute, query};
use crate::testing::mock_querier::mock_dependencies;

#[test]
fn add_margin() {
    let mut deps = mock_dependencies(&[]);

    let info = mock_info("owner", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("owner", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::RegisterMarket { contract_addr: "bank0000".to_string() };

    // Register Market
    let register_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::DepositStable { market_addr: "bank0000".to_string() };

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    // Add Margin
    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Position { market_addr: "bank0000".to_string(), user_addr: "depositor".to_string() }).unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();

    assert_eq!(position.margin, Uint256::from(100u128));

    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Position { market_addr: "bank0000".to_string(), user_addr: "depositor".to_string() }).unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();

    assert_eq!(position.margin, Uint256::from(200u128));

}


#[test]
fn open_position() {
    let mut deps = mock_dependencies(&[]);

    let info = mock_info("owner", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("owner", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::RegisterMarket { contract_addr: "bank0000".to_string() };

    // Register Market
    let register_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::DepositStable { market_addr: "bank0000".to_string() };

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    // Add Margin
    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Position { market_addr: "bank0000".to_string(), user_addr: "depositor".to_string() }).unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();

    assert_eq!(position.margin, Uint256::from(100u128));

    // Open Position

    let msg = ExecuteMsg::OpenPosition { market_addr: "bank0000".to_string(), open_value: Uint256::from(500u128), direction: Direction::LONG };

    let info = mock_info("depositor", &vec![]);

    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Position { market_addr: "bank0000".to_string(), user_addr: "depositor".to_string() }).unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();

    assert_eq!(position.openingValue, Uint256::from(500u128));
    assert_eq!(position.positionSize, Uint256::from(50u128));

}


#[test]
fn close_position() {
    let mut deps = mock_dependencies(&[]);

    let info = mock_info("owner", &vec![]);

    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
    };

    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("owner", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::RegisterMarket { contract_addr: "bank0000".to_string() };

    // 1. Register Market
    let register_res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let msg = ExecuteMsg::DepositStable { market_addr: "bank0000".to_string() };

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    // 2. Add Margin
    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Position { market_addr: "bank0000".to_string(), user_addr: "depositor".to_string() }).unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();

    assert_eq!(position.margin, Uint256::from(100u128));

    // 3. Open Position

    let msg = ExecuteMsg::OpenPosition { market_addr: "bank0000".to_string(), open_value: Uint256::from(500u128), direction: Direction::LONG };

    let info = mock_info("depositor", &vec![]);

    execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Position { market_addr: "bank0000".to_string(), user_addr: "depositor".to_string() }).unwrap();
    let position: PositionResponse = from_binary(&res).unwrap();

    assert_eq!(position.openingValue, Uint256::from(500u128));
    assert_eq!(position.positionSize, Uint256::from(50u128));
    assert_eq!(position.margin_left, Uint256::from(50u128));
    assert_eq!(position.current_value, Uint256::from(450u128));
    assert_eq!(position.pnl, -50i64);

    // 4. Close Position

    let msg = ExecuteMsg::ClosePosition { market_addr: "bank0000".to_string() };

    let info = mock_info("depositor", &vec![]);

    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();

    assert_eq!(res.messages[1], 
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "depositor".to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100u128 - (500u128 - 50u128 * 9u128))
            }]
        }))
    );

}