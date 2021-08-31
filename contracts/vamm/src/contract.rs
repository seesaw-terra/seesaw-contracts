use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, CanonicalAddr, Reply, StdError, attr, SubMsg, WasmMsg, ReplyOn, from_binary, Addr
};
use cosmwasm_bignumber::{Decimal256, Uint256};
use seesaw::bank::Direction;
use terraswap::asset::{AssetInfo};
use cw20::{ Cw20ReceiveMsg};
use seesaw::vamm::{ ConfigResponse, ExecuteMsg, InstantiateMsg, MarketsResponse, PositionResponse, QueryMsg, StateResponse};
use crate::state::{CONFIG, Config, STATE, State};

use crate::{ error::ContractError };

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {

    let config = Config {
        contract_addr: deps.api.addr_canonicalize(&env.contract.address.as_str())?,
        bank_addr: deps.api.addr_canonicalize(&msg.bank_addr.as_str())?,
        stable_denom: msg.stable_denom,
    };

    CONFIG.save(deps.storage, &config)?;

    let funding_period: u128 = 8 * 60 * 60 * 1000; // Every 8 hours

    let state = State {
        base_asset_reserve: msg.init_base_reserve, // Initialize at a certain price
        quote_asset_reserve: msg.init_quote_reserve, // Initialize at a certain price
        funding_period: Uint256::from(funding_period),
    };

    STATE.save(deps.storage, &state)?;
    
    Ok(Response::new().add_attributes(vec![("action", "instantiate")]))

}

// And declare a custom Error variant for the ones where you will want to make use of it
#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SwapIn { quote_asset_amount, direction } => swap_in(deps, env, info, quote_asset_amount, direction),
        ExecuteMsg::SwapOut { quote_asset_amount, direction } => swap_in(deps, env, info, quote_asset_amount, direction),
    }
}

pub fn swap_in (
    deps: DepsMut, 
    env: Env, 
    info: MessageInfo, 
    quote_asset_amount: Uint256, 
    direction: Direction
) -> Result<Response, ContractError> {

    let base_amount = get_base_from_quote(&deps, quote_asset_amount, &direction)?;

    let state: State = STATE.load(deps.storage)?;

    let mut new_state = state.clone();

    match direction {
        Direction::LONG => {
            new_state.base_asset_reserve += base_amount;
            new_state.quote_asset_reserve = state.quote_asset_reserve - quote_asset_amount;
        }
        Direction::SHORT => {
            new_state.quote_asset_reserve += quote_asset_amount;
            new_state.base_asset_reserve = state.base_asset_reserve - base_amount;
        }
        Direction::NOT_SET => {
            return Err(ContractError::Std(StdError::generic_err("Invalid Direction").into()));
        }
    }

    STATE.save(deps.storage, &new_state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "swap")
    ])
    )

}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::BaseFromQuote  { quoteAmount, direction } => to_binary(&query_config(deps)?),
        QueryMsg::QuoteFromBase { baseAmount, direction } => to_binary(&query_config(deps)?),
        QueryMsg::OraclePrice {} => to_binary(&query_config(deps)?),
        QueryMsg::MarketPrice {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?)
    }
}

pub fn get_base_from_quote(deps: &DepsMut, quoteAmount: Uint256, direction: &Direction ) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage)?;
    return get_base_from_quote_internal(quoteAmount, direction,  state.quote_asset_reserve, state.base_asset_reserve);
}


fn get_base_from_quote_internal( quoteAmount: Uint256, direction: &Direction, quote_reserve_amounts: Uint256, base_reserve_amounts: Uint256 ) -> StdResult<Uint256> {
    let k: Uint256 = quote_reserve_amounts * base_reserve_amounts; // x*y = k
    
    let mut new_quote_reserve = match direction {
        Direction::LONG => {
            quote_reserve_amounts - quoteAmount
        }
        Direction::SHORT => {
            quote_reserve_amounts + quoteAmount
        }
        Direction::NOT_SET => {
            return Err(StdError::generic_err("Invalid Direction").into());
        }
    };

    let new_base_reserve: Uint256 = k / Decimal256::from_uint256(new_quote_reserve);

    let base_reserve_delta = if new_base_reserve > base_reserve_amounts {
        new_base_reserve - base_reserve_amounts
    } else {
        base_reserve_amounts - new_base_reserve
    };

    Ok(base_reserve_delta)
}


pub fn get_quote_from_base(deps: DepsMut, baseAmount: Uint256, direction: Direction ) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage)?;
    return get_quote_from_base_internal(baseAmount, direction,  state.quote_asset_reserve, state.base_asset_reserve);
}


fn get_quote_from_base_internal( baseAmount: Uint256, direction: Direction, quote_reserve_amounts: Uint256, base_reserve_amounts: Uint256 ) -> StdResult<Uint256> {
    let k: Uint256 = quote_reserve_amounts * base_reserve_amounts; // x*y = k
    
    let mut new_base_reserve = match direction {
        Direction::LONG => {
            base_reserve_amounts - baseAmount // Longs will close position by trading in quote assets, and getting back base assets
        }
        Direction::SHORT => {
            base_reserve_amounts + baseAmount // Shorts will close position by trading in base assets, and getting back quote assets
        }
        Direction::NOT_SET => {
            return Err(StdError::generic_err("Invalid Direction").into());
        }
    };

    let new_quote_reserve: Uint256 = k / Decimal256::from_uint256(new_base_reserve);

    let quote_reserve_delta = if new_quote_reserve > quote_reserve_amounts {
        new_quote_reserve - quote_reserve_amounts
    } else {
        quote_reserve_amounts - new_quote_reserve
    };

    Ok(quote_reserve_delta)
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        contract_addr: deps.api.addr_humanize(&config.contract_addr)?,
        bank_addr: deps.api.addr_humanize(&config.bank_addr)?,
        stable_denom: config.stable_denom,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        quote_asset_reserve: state.quote_asset_reserve,
        base_asset_reserve: state.base_asset_reserve,
    })
}
