use cosmwasm_bignumber::{Uint256,Decimal256};
use cosmwasm_std::{ 
    DepsMut, Env, MessageInfo, Response, Deps, SubMsg, CosmosMsg, WasmMsg, Uint128, to_binary, attr, StdResult, Addr
};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::{query_supply,query_balance};
use cw20::{Cw20ExecuteMsg};

use crate::error::ContractError;
use crate::state::{ CONFIG, Config, STATE, State, POSITIONS, Position };

use seesaw::bank::{ Direction };

// Add Margin to a vAMM of selection
pub fn add_margin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_addr: Addr
) -> Result<Response, ContractError> {

    // load config 
    let config:Config = CONFIG.load(deps.storage)?;

    // check base denom deposit
    let deposit_amount: Uint256 = info
    .funds
    .iter()
    .find(|c| c.denom == config.stable_denom)
    .map(|c| Uint256::from(c.amount))
    .unwrap_or_else(Uint256::zero);
    // cannot deposit zero amount
    if deposit_amount.is_zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let positions_res  = POSITIONS.may_load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    match positions_res {
        None => {
            let new_position: Position = Position {
                positionSize: Uint256::zero(),
                openingValue: Uint256::zero(),
                direction: Direction::NOT_SET,
                margin: Uint256::zero()
            };
            POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position);
        }
        Some(position) => {
            let mut new_position = position;
            new_position.margin += deposit_amount;
            POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position);
        }
    };

    let messages: Vec<CosmosMsg> = vec![];

    Ok(Response::new().add_messages(messages)
        .add_attributes(vec![
            ("action", "add_margin"),
            ("amount_added", deposit_amount.to_string().as_str())
        ])
    )
}

// Add Margin to a vAMM of selection
pub fn open_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_addr: Addr,
    direction: Direction,
    openingValue: Uint256 // amount of assets to hold position on
) -> Result<Response, ContractError> {

    let position = POSITIONS.load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    if position.direction != Direction::NOT_SET {
        return Err(ContractError::PositionAlreadyOpen {});
    }
    
    /// PERFORM SWAP ON AMM

    let positionSize = Uint256::zero(); //WILL GET FROM AMM

    // END PERFORM SWAP ON AMM


    // Update position to reflect opened position
    let mut new_position = position.clone();
    new_position.openingValue = openingValue;
    new_position.positionSize = positionSize;
    new_position.direction = direction;
    let position = POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position)?;

    // Perform swap on AMM

    Ok(Response::default())
}

// pub fn withdraw_collateral(
//     deps: DepsMut,
//     env: Env,
//     _info: MessageInfo,
//     sender_addr: Addr,
//     amount: Uint256
// ) -> Result<Response, ContractError> {
//     // load config 
//     let config:Config = CONFIG.load(deps.storage)?;
//     // update interest rate
//     let mut state:State = STATE.load(deps.storage)?;

//     // cannot withdraw zero amount
//     if amount.is_zero() {
//         return Err(ContractError::InvalidZeroAmount {});
//     }

//     // calculate redeem rate
//     // let redeem_amount = compute_redeem_rate(deps.as_ref(), &config, &state, amount)?;
    
//     // update state
//     STATE.save(deps.storage, &state)?;
//     // send fund back to user
//     let mut messages: Vec<CosmosMsg> = vec![];
//     let asset = Asset {
//         info: AssetInfo::NativeToken {
//             denom: config.stable_denom
//         },
//         amount: Uint128::from(3u32)
//     };
//     messages.push(asset.clone().into_msg(&deps.querier, sender_addr)?);
//     // burn ib token

//     messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
//         contract_addr: deps.api.addr_humanize(&config.ib_token_addr)?.to_string(),
//         msg: to_binary(&Cw20ExecuteMsg::Burn {
//             amount: Uint128::from(amount),
//         })?,
//         funds: vec![]
//     }));
//     Ok(Response::new().add_messages(messages).add_attributes(
//         vec![
//             attr("action", "withdraw")
//         ])
//     )
// }