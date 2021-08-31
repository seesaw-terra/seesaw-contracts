use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_binary, Reply, ContractResult, SubMsgExecutionResponse};
use cosmwasm_bignumber::Decimal256;
use seesaw::bank::{InstantiateMsg, QueryMsg, ConfigResponse};

use crate::contract::{instantiate, query, reply};
use crate::testing::mock_querier::mock_dependencies;

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
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

    let query_res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&query_res).unwrap();
    assert_eq!("uusd",config_res.stable_denom);
    assert_eq!("liquidity0000", config_res.ib_token_addr);
    assert_eq!("owner", config_res.owner_addr);
}