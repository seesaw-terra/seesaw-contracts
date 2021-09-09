use cosmwasm_bignumber::{Uint256,Decimal256};
use cosmwasm_std::{Addr, Api, BankMsg, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo, Querier, QuerierWrapper, QueryRequest, Response, StdError, StdResult, Storage, SubMsg, Uint128, WasmMsg, WasmQuery, attr, to_binary};
use moneymarket::custody::Cw20HookMsg;
use terraswap::asset::{Asset, AssetInfo};
use terraswap::querier::{query_supply,query_balance};
use cw20::{Cw20ExecuteMsg};
use seesaw::vamm::{WhoPays, ExecuteMsg as VammExecuteMsg, QueryMsg as VammQueryMsg, StateResponse as VammStateResponse, Funding};
use moneymarket::market::{ExecuteMsg as AnchorExecuteMsg, QueryMsg as AnchorQueryMsg, ConfigResponse as AnchorConfigResponse, StateResponse as AnchorStateResponse, Cw20HookMsg as AnchorCw20HookMsg };

use crate::error::ContractError;
use crate::state::{ CONFIG, Config, STATE, State, POSITIONS, Position, MARKETS, Market };

use seesaw::bank::{Direction, FundingResponse, Sign};

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
                margin: deposit_amount,
                last_cumulative_funding: Decimal256::zero(),
                last_cumulative_long_rewards: Decimal256::zero(),
                last_cumulative_short_rewards: Decimal256::zero(),
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

    let mut messages: Vec<CosmosMsg> = vec![];

    // Send deposit to Anchor
    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: deps.api.addr_humanize(&config.anchor_addr)?.to_string(),
        msg: to_binary(&AnchorExecuteMsg::DepositStable { })?,
        funds: vec![Coin {
            amount: Uint128::from(deposit_amount),
            denom: "uusd".to_string()
        }],
    });

    messages.push(msg);

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
    let mut market = MARKETS.load(deps.storage, market_addr.as_bytes())?;

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
    
    let state: State = STATE.load(deps.storage)?;

    // 3. Update Pool's Anchor Reward Distribution
    update_anchor_rewards(deps.api, deps.storage, &deps.querier, market_addr.clone());

    // 4. Update Market total margins
    match direction {
        Direction::LONG => {
            market.total_long_margin += position.margin;
            market.total_margin += position.margin;
        },
        Direction::SHORT => {
            market.total_short_margin += position.margin;
            market.total_margin += position.margin;
        },
        Direction::NOT_SET => {
            // Throw error if invalid query
            return Err(ContractError::PositionNotOpen {});
        }
    }

    // 5. Update position to reflect opened position
    let mut new_position = position.clone();
    new_position.openingValue = open_value;
    new_position.positionSize = position_size;
    new_position.direction = direction;
    new_position.last_cumulative_funding = market.cumulative_funding_premium;
    new_position.last_cumulative_long_rewards = market.cumulative_long_rewards;
    new_position.last_cumulative_short_rewards = market.cumulative_short_rewards;

    POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position)?;
    MARKETS.save(deps.storage, market_addr.as_bytes(), &market);
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
    let mut market = MARKETS.load(deps.storage, market_addr.as_bytes())?;

    let position: Position = POSITIONS.load(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()))?;

    if position.direction == Direction::NOT_SET {
        return Err(ContractError::PositionNotOpen {});
    }

    // 1. Simulate Swap on AMM

    let config: Config = CONFIG.load(deps.storage)?;

    let (_,_,_,margin_adjusted) = simulate_close(deps.as_ref(), market_addr.clone(), position.clone())?;

    let mut messages: Vec<CosmosMsg> = vec![];

    /// 4. Perform Swap on vAMM
    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammExecuteMsg::SwapOut {
            direction: position.direction.clone(),
            base_asset_amount: position.positionSize
        })?,
        funds: vec![],
    });

    messages.push(msg);

    /// 5. Redeem Tokens from Anchor
    
    // Get current anchor exchange rate

    let anchor_config: AnchorConfigResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: deps.api.addr_humanize(&config.anchor_addr)?.to_string(),
        msg: to_binary(&AnchorQueryMsg::Config { })?,
    }))?;

    let anchor_state: AnchorStateResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: deps.api.addr_humanize(&config.anchor_addr)?.to_string(),
        msg: to_binary(&AnchorQueryMsg::State { block_height: None })?,
    }))?;

    // Get amount of aTokens equivalence of margin_adjusted
    let a_token_to_redeem = margin_adjusted * anchor_state.prev_exchange_rate;
    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: anchor_config.aterra_contract.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: deps.api.addr_humanize(&config.anchor_addr)?.to_string(),
            amount: Uint128::from(a_token_to_redeem), 
            msg: to_binary(&AnchorCw20HookMsg::RedeemStable {}

            )?
        })?,
        funds: vec![],
    });

    /// 6. Transfer back margin to user wallet.
    let msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.stable_denom,
            amount: Uint128::from(margin_adjusted),
        }],
    });

    messages.push(msg);

    // 7. Update Pool's Anchor Reward Distribution
    update_anchor_rewards(deps.api, deps.storage, &deps.querier, market_addr.clone());

    // 8. Update Market total margins
    match position.direction {
        Direction::LONG => {
            market.total_long_margin = market.total_long_margin - position.margin;
            market.total_margin = market.total_margin - position.margin;
        },
        Direction::SHORT => {
            market.total_short_margin = market.total_short_margin - position.margin;
            market.total_margin = market.total_margin - position.margin;
        },
        Direction::NOT_SET => {
            // Throw error if invalid query
            return Err(ContractError::PositionNotOpen {});
        }
    }
    
    // 9. Clear the position
    let mut new_position = Position {
        margin: Uint256::zero(),
        openingValue: Uint256::zero(),
        positionSize: Uint256::zero(),
        direction: Direction::NOT_SET,
        last_cumulative_funding: Decimal256::zero(),
        last_cumulative_long_rewards: Decimal256::zero(),
        last_cumulative_short_rewards: Decimal256::zero(),
    };

    POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position)?;
    MARKETS.save(deps.storage, market_addr.as_bytes(), &market);

    // 7. Send swap messages
    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "close position"),
            ("openingValue", new_position.openingValue.to_string().as_str()),
            ("positionSize", new_position.positionSize.to_string().as_str())
        ])
    )
}

// Add Margin to a vAMM of selection
pub fn simulate_close(
    deps: Deps,
    market_addr: Addr,
    position: Position
    // Returns PNL, New Position Size, MarginLeft
) -> StdResult<(i64, FundingResponse, Uint256, Uint256)> {

    let config: Config = CONFIG.load(deps.storage)?;

    // Get current position value
    let new_position_value: Uint256 = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammQueryMsg::SimulateOut { baseAmount: position.positionSize.clone(), direction: position.direction.clone() })?,
    }))?;

   // 2. Calculate funding fee realized
    let market: Market = MARKETS.load(deps.storage, &market_addr.as_bytes())?;

    let funding: Decimal256 = if market.cumulative_funding_premium > position.last_cumulative_funding {
        (market.cumulative_funding_premium - position.last_cumulative_funding) * Decimal256::from_uint256(position.positionSize)
    } else {
        (position.last_cumulative_funding - market.cumulative_funding_premium) * Decimal256::from_uint256(position.positionSize)
    };

    let mut funding_response: FundingResponse;

    // 3. Calculate how much user's funding fees are subsidized by anchor

    let (cumulative_long_rewards, cumulative_short_rewards) = get_current_anchor_cumulative_rewards(deps.api, deps.storage, &deps.querier, market_addr)?;

    // Get amount of anchor rewards distributed to user
    let anchor_help =  match position.direction {
        Direction::LONG => {
            (cumulative_long_rewards - position.last_cumulative_long_rewards) * position.margin
        },
        Direction::SHORT => {
            (cumulative_short_rewards - position.last_cumulative_short_rewards) * position.margin
        },
        Direction::NOT_SET => {
            return Err(StdError::GenericErr { msg: "UNSET DIRECTION".to_string() });
        },
    };

    // 4. Calculate margin with pnl and funding realized
    let margin_funding_pnl_adjusted: Uint256 = match position.direction {
        Direction::LONG => {
            // margin_pnl_adjusted = old_margin + (curr_value - open_value) = old_margin - open_value + curr_value
            let intermediary1 = position.margin + new_position_value;
            let intermediary_anchor_help = intermediary1 + anchor_help;

            let intermediary2 = if market.cumulative_funding_premium > position.last_cumulative_funding {
                funding_response = FundingResponse {
                    amount: funding * Uint256::one(),
                    sign: Sign::Negative
                };
                safe_subtract_min_zero(intermediary_anchor_help, funding * Uint256::one()) // If funding premium increased, pays
            } else {
                funding_response = FundingResponse {
                    amount: funding * Uint256::one(),
                    sign: Sign::Positive
                };
                intermediary_anchor_help + funding * Uint256::one() // If funding premium decreased, gets paid
            };
            
            safe_subtract_min_zero(intermediary2, position.openingValue)

        },
        Direction::SHORT => {

            let intermediary1 = position.margin + position.openingValue;
            let intermediary_anchor_help = intermediary1 + anchor_help;

            let intermediary2 = if market.cumulative_funding_premium > position.last_cumulative_funding {
                funding_response = FundingResponse {
                    amount: funding * Uint256::one(),
                    sign: Sign::Positive
                };
                intermediary_anchor_help + funding * Uint256::one() // If funding premium increased, gets paid
            } else {
                funding_response = FundingResponse {
                    amount: funding * Uint256::one(),
                    sign: Sign::Negative
                };
                safe_subtract_min_zero(intermediary_anchor_help, funding * Uint256::one()) // If funding premium decreased, pays
            };
            
            safe_subtract_min_zero(intermediary2, new_position_value)
        },
        Direction::NOT_SET => {
            return Err(StdError::GenericErr { msg: "UNSET DIRECTION".to_string() });
        },
    };

    // Convert all to i64
    let signed_curr_value: i64 = u128::from(new_position_value) as u64 as i64;
    let signed_open_value: i64 = u128::from(position.openingValue) as u64 as i64;

    // Calculate PNL
    let pnl: i64 = match position.direction {
        Direction::LONG => {
            signed_curr_value - signed_open_value
        },
        Direction::SHORT => {
            signed_open_value - signed_curr_value
        },
        Direction::NOT_SET => {
            return Err(StdError::GenericErr { msg: "UNSET DIRECTION".to_string() });
        },
    };

    Ok((pnl,funding_response,new_position_value,margin_funding_pnl_adjusted))

}


// Add Margin to a vAMM of selection
pub fn liquidate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    market_addr: Addr,
    holder_addr: Addr
) -> Result<Response, ContractError> {

    // Crash if market doesn't exist
    let market = MARKETS.load(deps.storage, market_addr.as_bytes());

    let position: Position = POSITIONS.load(deps.storage, (market_addr.as_bytes(), holder_addr.as_bytes()))?;

    if position.direction == Direction::NOT_SET {
        return Err(ContractError::PositionNotOpen {});
    }

    // 1. Simulate Swap on AMM

    let config: Config = CONFIG.load(deps.storage)?;

    let (_,_,_,margin_adjusted) = simulate_close(deps.as_ref(), market_addr.clone(), position.clone())?; //  Get current margin

    // 2. Check ratio

    let margin_ratio: Decimal256 = Decimal256::from_ratio(margin_adjusted, position.openingValue);

    // if current ratio is outside liquidation threshhold, throw error
    if margin_ratio > config.liquidation_ratio {
        return Err(ContractError::Unliquidatable {});
    }

    /// 3. Perform Swap on vAMM
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

    // 4. Split margin for liquidators and for users
    let margin_to_liquidators = if position.openingValue * config.liquidation_reward > margin_adjusted {
        position.openingValue * config.liquidation_reward
    } else {
        margin_adjusted
    };

    let margin_to_holders = margin_adjusted - margin_to_liquidators;


    /// 5. Transfer margin to liquidators wallet.
    let msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: vec![Coin {
            denom: config.stable_denom.clone(),
            amount: Uint128::from(margin_to_liquidators),
        }],
    });
    
    /// 6. Transfer margin to holders wallet.
    let msg: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: holder_addr.to_string(),
        amount: vec![Coin {
            denom: config.stable_denom.clone(),
            amount: Uint128::from(margin_to_holders),
        }],
    });

    messages.push(msg);
    
    // 7. Clear the position
    let mut new_position = Position {
        margin: Uint256::zero(),
        openingValue: Uint256::zero(),
        positionSize: Uint256::zero(),
        direction: Direction::NOT_SET,
        last_cumulative_funding: Decimal256::zero(),
        last_cumulative_long_rewards: Decimal256::zero(),
        last_cumulative_short_rewards: Decimal256::zero(),
    };

    POSITIONS.save(deps.storage, (market_addr.as_bytes(), info.sender.as_bytes()), &new_position)?;

    // 8. Send messages
    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "close position"),
            ("openingValue", new_position.openingValue.to_string().as_str()),
            ("positionSize", new_position.positionSize.to_string().as_str())
        ])
    )
}


pub fn get_anchor_cumulative(
    api: &dyn Api,
    storage: &dyn Storage,
    querier: &QuerierWrapper
) -> StdResult<Decimal256> {
    // Get Current Anchor Interest Index
    let config = CONFIG.load(storage)?;

    let anchor_state: AnchorStateResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: api.addr_humanize(&config.anchor_addr)?.to_string(),
        msg: to_binary(&AnchorQueryMsg::State { block_height: None })?,
    }))?;

    Ok(anchor_state.global_interest_index)
}

fn get_current_anchor_cumulative_rewards(api: &dyn Api, storage: &dyn Storage, querier: &QuerierWrapper, market_addr: Addr) -> StdResult<(Decimal256, Decimal256)> {
    let vamm_state: VammStateResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammQueryMsg::VammState {  })?,
    }))?;

    let mut market: Market = MARKETS.load(storage, &market_addr.as_bytes())?;

    let curr_anchor_index = get_anchor_cumulative(api, storage, &querier)?;

    match vamm_state.funding_fee.who_pays {
        WhoPays::LONG => {
            // IF Longs are paying, help them pay with pool rewards
            if market.total_long_margin > Uint256::zero() {
                market.cumulative_long_rewards += Decimal256::from_uint256(curr_anchor_index / market.last_anchor_index * market.total_margin / Decimal256::from_uint256(market.total_long_margin));
            }
        },
        WhoPays::SHORT => {
            // IF Shorts are paying, help them pay with pool rewards
            if market.total_short_margin > Uint256::zero() {
                market.cumulative_short_rewards += Decimal256::from_uint256(curr_anchor_index / market.last_anchor_index * market.total_margin / Decimal256::from_uint256(market.total_short_margin));
            }
        }
    }
    Ok((market.cumulative_long_rewards, market.cumulative_short_rewards))
}

pub fn update_anchor_rewards(api: &dyn Api, storage: &mut dyn Storage, querier: &QuerierWrapper, market_addr: Addr) -> Result<Response, ContractError>{
    let mut market: Market = MARKETS.load(storage, &market_addr.as_bytes())?;

    let (cumulative_long_rewards, cumulative_short_rewards) = get_current_anchor_cumulative_rewards(api, storage, querier, market_addr.clone())?;

    let curr_anchor_index = get_anchor_cumulative(api, storage, &querier)?;

    market.cumulative_long_rewards = cumulative_long_rewards;
    market.cumulative_short_rewards = cumulative_short_rewards;
    market.last_anchor_index = curr_anchor_index;

    MARKETS.save(storage, &market_addr.as_bytes(), &market)?;

    Ok(Response::default())
}






