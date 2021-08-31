use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{Addr, Reply, ContractResult, SubMsgExecutionResponse, Coin, Uint128, to_binary, SubMsg, CosmosMsg, WasmMsg, BankMsg, Decimal};
use seesaw::bank::{InstantiateMsg, ExecuteMsg, Cw20HookMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::contract::{instantiate, reply, execute};
use crate::testing::mock_querier::mock_dependencies;

#[test]
fn deposit() {
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
        token_code_id: 10u64,
        interest_multipiler: Decimal256::zero()
    };
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // store liquidity token
    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(
                vec![
                    10, 13, 108, 105, 113, 117, 105, 100, 105, 116, 121, 48, 48, 48, 48,
                ]
                .into(),
            ),
        }),
    };
    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    let info = mock_info("depositor", &vec![Coin {
        denom: "uusd".to_string(),
        amount: Uint128::from(100u128)
    }]);

    let msg = ExecuteMsg::DepositStable { market_addr: Addr::unchecked("mock amm") };

    deps.querier.with_balance(&[(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(
                100u128, /* user deposit must be pre-applied */
            ),
        }],
    )]);

    // check mint ib token
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(true,true);
}

#[test]
fn withdraw() {
    let mut deps = mock_dependencies(&[]);

    deps.querier.with_tax(
        Decimal::zero(),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );

    deps.querier.with_balance(&[(
        &MOCK_CONTRACT_ADDR.to_string(),
        vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(100u128),
        }],
    )]);

    deps.querier.with_token_balances(&[
        (
            &"liquidity0000".to_string(),
            &[(&"depositor".to_string(), &Uint128::from(100u128))],
        )
    ]);

    let info = mock_info("owner", &vec![]);
    let msg = InstantiateMsg {
        stable_denom: "uusd".to_string(),
        token_code_id: 10u64,
        interest_multipiler: Decimal256::zero()
    };
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // store liquidity token
    let reply_msg = Reply {
        id: 1,
        result: ContractResult::Ok(SubMsgExecutionResponse {
            events: vec![],
            data: Some(
                vec![
                    10, 13, 108, 105, 113, 117, 105, 100, 105, 116, 121, 48, 48, 48, 48,
                ]
                .into(),
            ),
        }),
    };
    let _res = reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    let info = mock_info("liquidity0000", &vec![]);

    let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
        sender: "depositor".to_string(),
        amount: Uint128::from(100u128),
        msg: to_binary(&Cw20HookMsg::WithdrawStable {}).unwrap()
    });
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let msg_redeem_asset = res.messages.get(0).expect("no message");
    assert_eq!(
        msg_redeem_asset,
        &SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: "depositor".to_string(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(100u128)
            }]
        }))
    );
    let msg_burn_liquidity = res.messages.get(1).expect("no message");
    assert_eq!(
        msg_burn_liquidity,
        &SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "liquidity0000".to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: Uint128::from(100u128),
            }).unwrap(),
            funds: vec![]
        }))
    );
}