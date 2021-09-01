use cosmwasm_bignumber::{Uint256,Decimal256};
use cosmwasm_std::{Addr, CosmosMsg, Deps, DepsMut, Env, MessageInfo, QueryRequest, Response, StdResult, SubMsg, Uint128, WasmMsg, WasmQuery, attr, to_binary};
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::{query_supply,query_balance};
use cw20::{Cw20ExecuteMsg};
use seesaw::vamm::{ExecuteMsg as VammExecuteMsg, QueryMsg as VammQueryMsg };

use crate::error::ContractError;
use crate::state::{ CONFIG, Config, STATE, State, POSITIONS, Position, MARKETS, Market };

use seesaw::bank::{ Direction };

// Add Margin to a vAMM of selection
pub fn add_margin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_addr: Addr
) -> Result<Response, ContractError> {

    // Crash if market doesn't exist
    let market = MARKETS.load(deps.storage, market_addr.as_bytes());

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
    open_value: Uint256 // Value of position that would like to open at, eg. 10,000 UST
) -> Result<Response, ContractError> {

    // Crash if market doesn't exist
    let market = MARKETS.load(deps.storage, market_addr.as_bytes());

    let position = POSITIONS.load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    if position.direction != Direction::NOT_SET {
        return Err(ContractError::PositionAlreadyOpen {});
    }
    
    /// 1. Perform Swap on AMM

    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammExecuteMsg::SwapIn {
            direction: direction.clone(),
            quote_asset_amount: open_value
        })?,
        funds: vec![],
    });

    let mut messages: Vec<CosmosMsg> = vec![];
    messages.push(msg);

    // 2. Simulate Swap on AMM

    let config: Config = CONFIG.load(deps.storage)?;

    let position_size: Uint256 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammQueryMsg::SimulateIn { quoteAmount: open_value, direction: direction.clone(), })?,
    }))?;
    
    // 3. Update position to reflect opened position
    let mut new_position = position.clone();
    new_position.openingValue = open_value;
    new_position.positionSize = position_size;
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

fn safe_subtract_min_zero(left: Uint256, right: Uint256) -> Uint256{
    if left > right {
        return left - right
    } else {
        return Uint256::zero()
    }
}

// Add Margin to a vAMM of selection
pub fn close_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_addr: Addr
) -> Result<Response, ContractError> {

    // Crash if market doesn't exist
    let market = MARKETS.load(deps.storage, market_addr.as_bytes());

    let position: Position = POSITIONS.load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    if position.direction == Direction::NOT_SET {
        return Err(ContractError::PositionNotOpen {});
    }

    // 1. Simulate Swap on AMM

    let config: Config = CONFIG.load(deps.storage)?;

    // Get current position value
    let new_position_value: Uint256 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammQueryMsg::SimulateOut { baseAmount: position.positionSize.clone(), direction: position.direction.clone() })?,
    }))?;

    // 2. Calculate margin with pnl realized
    let margin_pnl_adjusted: Uint256 = match position.direction {
        Direction::LONG => {
            // margin_pnl_adjusted = old_margin + (curr_value - open_value) = old_margin - open_value + curr_value
            safe_subtract_min_zero(position.margin + new_position_value, position.openingValue)
        },
        Direction::SHORT => {
            // margin_pnl_adjusted = old_margin + (open_value - curr_value) = old_margin + open_value - curr_value
            safe_subtract_min_zero(position.margin + position.openingValue, new_position_value)
        },
        Direction::NOT_SET => {
            return Err(ContractError::PositionNotOpen {});
        },
    };

    // 3. Calculate margin with pnl and funding fee realized
    // TO DO: find a way to implement funding fee
    let margin_funding_adjusted: Uint256 = margin_pnl_adjusted;

    /// 4. Perform Swap on vAMM
    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammExecuteMsg::SwapOut {
            direction: position.direction.clone(),
            base_asset_amount: position.positionSize
        })?,
        funds: vec![],
    });

    let mut messages: Vec<CosmosMsg> = vec![];
    messages.push(msg);
    
    // 5. Clear the position
    let mut new_position = position.clone();
    new_position.openingValue = Uint256::zero();
    new_position.positionSize = Uint256::zero();
    new_position.direction = Direction::NOT_SET;
    POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position)?;

    // 6. Send swap messages
    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "close position"),
            ("openingValue", new_position.openingValue.to_string().as_str()),
            ("positionSize", new_position.positionSize.to_string().as_str())
        ])
    )
}


