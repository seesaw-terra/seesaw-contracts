use cosmwasm_bignumber::{Uint256,Decimal256};
use cosmwasm_std::{Addr, CosmosMsg, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, StdResult, SubMsg, Uint128, WasmMsg, WasmQuery, attr, to_binary};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::{query_supply,query_balance};
use cw20::{Cw20ExecuteMsg};
use seesaw::vamm::{ExecuteMsg as VammExecuteMsg, QueryMsg as VammQueryMsg };

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

    //  1. Load Config
    let config:Config = CONFIG.load(deps.storage)?;

    //  2. Get amount of deposited stable_denoms
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

    //  3. Load previous position, if new user, create new position

    let positions_res  = POSITIONS.may_load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    match positions_res {
        None => {
            //  4a. Create new position and add margin
            let new_position: Position = Position {
                positionSize: Uint256::zero(),
                openingValue: Uint256::zero(),
                direction: Direction::NOT_SET,
                margin: Uint256::zero()
            };
            POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position);
        }
        Some(position) => {
            //  4b. Load previous position and add margin
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
    positionSize: Uint256 // amount of assets to hold position on
) -> Result<Response, ContractError> {

    let position = POSITIONS.load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    if position.direction != Direction::NOT_SET {
        return Err(ContractError::PositionAlreadyOpen {});
    }
    
    /// 1. Perform Swap on AMM

    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammExecuteMsg::SwapIn {
            direction: direction.clone(),
            quote_asset_amount: positionSize
        })?,
        funds: vec![],
    });

    let mut messages: Vec<CosmosMsg> = vec![];
    messages.push(msg);

    // 2. Simulate Swap on AMM

    let config: Config = CONFIG.load(deps.storage)?;

    let openingValue: Uint256 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammQueryMsg::BaseFromQuote { quoteAmount: positionSize, direction: direction.clone(), })?,
    }))?;
    
    // 3. Update position to reflect opened position
    let mut new_position = position.clone();
    new_position.openingValue = openingValue;
    new_position.positionSize = positionSize;
    new_position.direction = direction;
    POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position)?;

    // 4. Send swap messages

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "open position"),
            ("openingValue", new_position.openingValue.to_string().as_str()),
            ("positionSize", new_position.positionSize.to_string().as_str())
        ])
    )
}
