use std::collections::VecDeque;
use std::time;

use crate::state::{CONFIG, Config, OracleType, STATE, State, SNAPSHOTS, MarketSnapshots, SnapshotItem};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, Binary, CanonicalAddr, Decimal, Deps, DepsMut, Env, MessageInfo, QuerierWrapper, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, Uint128, WasmMsg, attr, entry_point, from_binary, to_binary};
use cw20::Cw20ReceiveMsg;
use seesaw::bank::Direction;
use seesaw::vamm::{ConfigResponse, ExecuteMsg, Funding, InstantiateMsg, MarketItem, MarketsResponse, PositionResponse, QueryMsg, StateResponse, WhoPays};
use terra_cosmwasm::{ExchangeRatesResponse, TerraQuerier};
use terraswap::asset::AssetInfo;

use crate::error::ContractError;

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
        oracle_type: OracleType::NATIVE,
        base_denom: "uluna".to_string(),
    };

    CONFIG.save(deps.storage, &config)?;

    let funding_period: u128 = 8 * 60 * 60 * 1000; // Every 8 hours

    let state = State {
        base_asset_reserve: Uint256::from(msg.init_base_reserve), // Initialize at a certain price
        quote_asset_reserve: Uint256::from(msg.init_quote_reserve), // Initialize at a certain price
        funding_period: Uint256::from(funding_period), // Funding period in Nanoseconds
        aggregated_funding: Decimal256::from_uint256(Uint256::from(1_000_000_000u128)),
        funding_rate: Funding {
            amount: Decimal256::zero(),
            who_pays: WhoPays::LONG
        }
    };

    STATE.save(deps.storage, &state)?;

    let new_market_snapshots: MarketSnapshots = MarketSnapshots {
        snapshots: vec! [
            SnapshotItem {
                base_asset_reserve: Uint256::from(msg.init_base_reserve), 
                quote_asset_reserve: Uint256::from(msg.init_quote_reserve),
                base_delta: 0i64,
                timestamp: env.block.time.nanos()
            }
        ]
    };

    SNAPSHOTS.save(deps.storage, &new_market_snapshots)?;
    

    Ok(Response::new().add_attributes(vec![("action", "instantiate")]))
}

fn  create_snapshots(
    deps: Deps,
    env: &Env,
    base_asset_reserve: Uint256,
    quote_asset_reserve: Uint256, // Initialize at a certain price
    base_delta: i64,
) -> StdResult<MarketSnapshots> {
    let market_snapshots: MarketSnapshots = SNAPSHOTS.load(deps.storage)?;

    let snapshot_items: Vec<SnapshotItem> = market_snapshots.snapshots;
    let mut snapshot_deque = VecDeque::from(snapshot_items);

    snapshot_deque.push_back(
        SnapshotItem {
            base_asset_reserve: base_asset_reserve, // Initialize at a certain price
            quote_asset_reserve: quote_asset_reserve, // Initialize at a certain price
            base_delta: base_delta,
            timestamp: env.block.time.nanos()
        }
    );

    let new_market_snapshots: MarketSnapshots = MarketSnapshots {
        snapshots: Vec::from(snapshot_deque)
    };
    
    Ok(new_market_snapshots)
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
        ExecuteMsg::SwapIn {
            quote_asset_amount,
            direction,
        } => swap_in(deps, env, info, quote_asset_amount, direction),
        ExecuteMsg::SwapOut {
            base_asset_amount,
            direction,
        } => swap_out(deps, env, info, base_asset_amount, direction),
        ExecuteMsg::SettleFunding {} => settle_funding(deps, env, info),
    }
}

/*
    Settle Funding Function
*/

pub fn settle_funding(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {

    let config: Config = CONFIG.load(deps.storage)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.bank_addr {
        return Err(ContractError::Unauthorized {});
    }

    let spot_price = get_underlying_price(deps.as_ref())?;
    let mark_price = get_market_price(deps.as_ref())?;

    let state: State = STATE.load(deps.storage)?;

    let millis_day: u128 = 24 * 60 * 60 * 1000;

    // let premium: Decimal256 = spot_price - mark_price;

    let mut new_state = state.clone();

    let (premium, who_pays) = if spot_price > mark_price {
        (spot_price - mark_price, WhoPays::SHORT)
    } else {
        (mark_price - spot_price, WhoPays::LONG)
    };

    let premium_fraction: Decimal256 = premium * Decimal256::from_uint256(state.funding_period) / Decimal256::from_uint256(millis_day);

    new_state.aggregated_funding = match who_pays {
        WhoPays::SHORT => {
            new_state.aggregated_funding - premium_fraction
        },
        WhoPays::LONG => {
            new_state.aggregated_funding + premium_fraction
        }
    };

    new_state.funding_rate = Funding {
        amount: premium_fraction/spot_price,
        who_pays: who_pays // SHORT PAY LONGS
    };

    STATE.save(deps.storage, &new_state)?;

    Ok(Response::default())
}

fn get_market_price(deps: Deps) -> StdResult<Decimal256> {
    let state: State = STATE.load(deps.storage)?;
    let mark_price: Decimal256 = Decimal256::from_uint256(state.quote_asset_reserve)
        / Decimal256::from_uint256(state.base_asset_reserve);
    Ok(mark_price)
}

/*
    SWAP IN/OUT FUNCTIONS
*/

pub fn swap_in(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    quote_asset_amount: Uint256,
    direction: Direction,
) -> Result<Response, ContractError> {
    // Get amount of base we will be long/short
    // LONG -> how much base asset returned when we open position
    // SHORT -> how much base asset we borrow when we open position
    let base_amount = simulate_swapin(deps.as_ref(), quote_asset_amount, &direction)?;

    let state: State = STATE.load(deps.storage)?;

    let mut new_state = state.clone();

    match direction {
        Direction::LONG => {
            new_state.quote_asset_reserve += quote_asset_amount; // Send UST into market
            new_state.base_asset_reserve = state.base_asset_reserve - base_amount;

            let delta : i64 = u128::from(Uint128::from(base_amount)) as i64;

            let new_market_snapshots = create_snapshots(deps.as_ref(), &env, new_state.quote_asset_reserve, new_state.base_asset_reserve, delta)?;
            SNAPSHOTS.save(deps.storage, &new_market_snapshots)?;
        }
        Direction::SHORT => {
            new_state.base_asset_reserve += base_amount; // Sell borrowed base assets to market
            new_state.quote_asset_reserve = state.quote_asset_reserve - quote_asset_amount;
            // Take out UST from market

            let mut delta : i64 = u128::from(Uint128::from(base_amount)) as i64;
            delta = -delta; // Negative delta if short

            let new_market_snapshots = create_snapshots(deps.as_ref(), &env, new_state.quote_asset_reserve, new_state.base_asset_reserve, delta)?;
            SNAPSHOTS.save(deps.storage, &new_market_snapshots)?;
        }
        Direction::NOT_SET => {
            return Err(ContractError::Std(
                StdError::generic_err("Invalid Direction").into(),
            ));
        }
    }

    STATE.save(deps.storage, &new_state)?;

    Ok(Response::new().add_attributes(vec![("action", "swap")]))
}

pub fn swap_out(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    base_asset_amount: Uint256,
    direction: Direction,
) -> Result<Response, ContractError> {
    // Get amount of base we will be long/short
    // LONG -> how much base asset returned when we open position
    // SHORT -> how much base asset we borrow when we open position
    let quote_asset_amount = simulate_swapout(deps.as_ref(), base_asset_amount, &direction)?;

    let state: State = STATE.load(deps.storage)?;

    let mut new_state = state.clone();

    match direction {
        Direction::LONG => {
            new_state.base_asset_reserve += base_asset_amount; // Sell base assets to market
            new_state.quote_asset_reserve = state.quote_asset_reserve - quote_asset_amount;
            // Get UST back

            let mut delta : i64 = u128::from(Uint128::from(base_asset_amount)) as i64;
            delta = -delta; // Negative delta on closing if long

            let new_market_snapshots = create_snapshots(deps.as_ref(), &env, new_state.quote_asset_reserve, new_state.base_asset_reserve, delta)?;
            SNAPSHOTS.save(deps.storage, &new_market_snapshots)?;

        }
        Direction::SHORT => {
            new_state.quote_asset_reserve += quote_asset_amount; // Send UST into market
            new_state.base_asset_reserve = state.base_asset_reserve - base_asset_amount;
            // Buy base assets to return

            let mut delta : i64 = u128::from(Uint128::from(base_asset_amount)) as i64;
            delta = -delta; // Positive delta on closing if short

            let new_market_snapshots = create_snapshots(deps.as_ref(), &env, new_state.quote_asset_reserve, new_state.base_asset_reserve, delta)?;
            SNAPSHOTS.save(deps.storage, &new_market_snapshots)?;

        }
        Direction::NOT_SET => {
            return Err(ContractError::Std(
                StdError::generic_err("Invalid Direction").into(),
            ));
        }
    }

    STATE.save(deps.storage, &new_state)?;

    Ok(Response::new().add_attributes(vec![("action", "swap")]))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::SimulateIn {
            quoteAmount,
            direction,
        } => to_binary(&simulate_swapin(deps, quoteAmount, &direction)?),
        QueryMsg::SimulateOut {
            baseAmount,
            direction,
        } => to_binary(&simulate_swapout(deps, baseAmount, &direction)?),
        QueryMsg::OraclePrice {} => to_binary(&query_config(deps)?),
        QueryMsg::MarketPrice {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::MarketInfo {} => to_binary(&query_state(deps)?),
        QueryMsg::MarketSnapshots {} => to_binary(&get_market_snapshots(deps)?),
    }
}

/*
    ORACLE FUNCTIONS
*/

// Get the price of underlying asset
pub fn get_market_snapshots(deps: Deps) -> StdResult<MarketSnapshots> {
    let market_snapshots = SNAPSHOTS.load(deps.storage)?;

    return Ok(market_snapshots);
}

/*
    AMM SIMULATION FUNCTIONS
*/

pub fn simulate_swapin(
    deps: Deps,
    quoteAmount: Uint256,
    direction: &Direction,
) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage)?;
    return simulate_swapin_internal(
        quoteAmount,
        direction,
        state.quote_asset_reserve,
        state.base_asset_reserve,
    );
}

fn simulate_swapin_internal(
    quoteAmount: Uint256,
    direction: &Direction,
    quote_reserve_amounts: Uint256,
    base_reserve_amounts: Uint256,
) -> StdResult<Uint256> {
    let k: Uint256 = quote_reserve_amounts * base_reserve_amounts; // x*y = k
    let mut new_quote_reserve = match direction {
        Direction::LONG => quote_reserve_amounts + quoteAmount,
        Direction::SHORT => quote_reserve_amounts - quoteAmount,
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

pub fn simulate_swapout(
    deps: Deps,
    baseAmount: Uint256,
    direction: &Direction,
) -> StdResult<Uint256> {
    let state = STATE.load(deps.storage)?;
    return simulate_swapout_internal(
        baseAmount,
        direction,
        state.quote_asset_reserve,
        state.base_asset_reserve,
    );
}

fn simulate_swapout_internal(
    baseAmount: Uint256,
    direction: &Direction,
    quote_reserve_amounts: Uint256,
    base_reserve_amounts: Uint256,
) -> StdResult<Uint256> {
    let k: Uint256 = quote_reserve_amounts * base_reserve_amounts; // x*y = k
    let mut new_base_reserve = match direction {
        Direction::LONG => {
            base_reserve_amounts + baseAmount // Longs will close position by trading in base assets, and getting back quote assets
        }
        Direction::SHORT => {
            base_reserve_amounts - baseAmount // Shorts will close position by trading in quote assets, and getting back base assets
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

/*
    ORACLE FUNCTIONS
*/

// Get the price of underlying asset
pub fn get_underlying_price(deps: Deps) -> StdResult<Decimal256> {
    let config: Config = CONFIG.load(deps.storage)?;

    match config.oracle_type {
        OracleType::NATIVE => {
            return query_native_rate(&deps.querier, config.base_denom, config.stable_denom);
        } // MIRROR, BAND
    }
}

// NATIVE ORACLE
fn query_native_rate(
    querier: &QuerierWrapper,
    base_denom: String,
    quote_denom: String,
) -> StdResult<Decimal256> {
    let terra_querier = TerraQuerier::new(querier);
    let res: ExchangeRatesResponse =
        terra_querier.query_exchange_rates(base_denom, vec![quote_denom])?;

    Ok(Decimal256::from(res.exchange_rates[0].exchange_rate))
}

/*
    QUERY FUNCTIONS
*/

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
        funding_premium_cumulative: state.aggregated_funding,
        funding_fee: state.funding_rate,
        market_price: get_market_price(deps)?,
        underlying_price: get_underlying_price(deps)?
    })
}
