use cosmwasm_std::{Addr, Binary, CanonicalAddr, CosmosMsg, Deps, DepsMut, Env, MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, SubMsg, WasmMsg, WasmQuery, attr, entry_point, from_binary, to_binary};
use cosmwasm_bignumber::{Decimal256, Uint256};
use protobuf::Message;
use terraswap::asset::{AssetInfo};
use terraswap::token::InstantiateMsg as TokenInstantiateMsg;
use cw20::{MinterResponse, Cw20ReceiveMsg};
use seesaw::bank::{BorrowRateResponse, ConfigResponse, Cw20HookMsg, Direction, ExecuteMsg, FundingResponse, InstantiateMsg, MarketResponse, PositionResponse, QueryMsg, Sign, StateResponse};
use seesaw::vamm::{ExecuteMsg as VammExecuteMsg, QueryMsg as VammQueryMsg, StateResponse as VammStateResponse};
use moneymarket::market::{QueryMsg as AnchorQueryMsg, StateResponse as AnchorStateResponse};

use crate::error::ContractError;
use crate::state::{ CONFIG, Config, POSITIONS, Position, STATE, State, MARKETS, Market };
use crate::response::MsgInstantiateContractResponse;
use crate::positions::{add_margin, close_position, liquidate, open_position, simulate_close};

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
        owner_addr: deps.api.addr_canonicalize(&info.sender.as_str())?,
        anchor_addr: deps.api.addr_canonicalize(&msg.anchor_addr.as_str())?,
        stable_denom: msg.stable_denom,
        liquidation_ratio: msg.liquidation_ratio,
        liquidation_reward: msg.liquidation_reward,
    };
    CONFIG.save(deps.storage, &config)?;

    let state = State { };

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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env,info, msg),
        ExecuteMsg::DepositStable { market_addr } => {
            let valid_addr: Addr = deps.api.addr_validate(&market_addr.as_str())?;
            add_margin(deps, env, info, valid_addr) 
        },
        ExecuteMsg::RegisterMarket { contract_addr } => { 
            let valid_addr: Addr = deps.api.addr_validate(&contract_addr.as_str())?;
            register_market(deps, env, info, valid_addr)
        },
        ExecuteMsg::OpenPosition { market_addr, open_value, direction  } => {
            let valid_addr: Addr = deps.api.addr_validate(&market_addr.as_str())?;
            open_position(deps, env, info, valid_addr, direction, open_value)
        },
        ExecuteMsg::ClosePosition { market_addr } => {
            let valid_addr: Addr = deps.api.addr_validate(&market_addr.as_str())?;
            close_position(deps, env, info, valid_addr)
        },
        ExecuteMsg::Liquidate { market_addr, holder_addr } => {
            let valid_market_addr: Addr = deps.api.addr_validate(&market_addr.as_str())?;
            let valid_holder_addr: Addr = deps.api.addr_validate(&holder_addr.as_str())?;
            liquidate(deps, env, info, valid_market_addr, valid_holder_addr )
        },
        ExecuteMsg::UpdateFunding { market_addr } => {
            let valid_addr: Addr = deps.api.addr_validate(&market_addr.as_str())?;
            update_funding(deps, env, info, valid_addr)
        },
        ExecuteMsg::UpdateFundingInternal { market_addr } => {
            let valid_addr: Addr = deps.api.addr_validate(&market_addr.as_str())?;
            update_funding_internal(deps, env, info, valid_addr)
        }
    }
}

pub fn update_funding(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    market_addr: Addr,
) -> Result<Response, ContractError> {

    let mut messages: Vec<CosmosMsg> = vec![];

    /// 1. Prompt update to cumulative funding fraction.
    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammExecuteMsg::SettleFunding {})?,
        funds: vec![],
    });

    messages.push(msg);

    /// 2. Save funding fraction.
    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: _env.contract.address.to_string(),
        msg: to_binary(&ExecuteMsg::UpdateFundingInternal { market_addr: market_addr.to_string() })?,
        funds: vec![],
    });

    messages.push(msg);

    Ok(Response::new().add_messages(messages))
}

pub fn update_funding_internal(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    market_addr: Addr,
) -> Result<Response, ContractError> {
    let market: Market = MARKETS.load(deps.storage, market_addr.as_bytes())?;

    let mut new_market: Market = market.clone();

    let state: VammStateResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: market_addr.to_string(),
        msg: to_binary(&VammQueryMsg::VammState { })?,
    }))?;

    new_market.cumulative_funding_premium = state.funding_premium_cumulative;

    MARKETS.save(deps.storage, market_addr.as_bytes(), &new_market);

    Ok(Response::default())
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::WithdrawStable {}) => {
            let sender_addr = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            let contract_addr = deps.api.addr_canonicalize(info.sender.as_str())?;
            let config = CONFIG.load(deps.storage)?;

            Ok(Response::default())
        }
        Err(err) => Err(ContractError::Std(err))
    }
}

pub fn register_market(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    contract_addr: Addr,
) -> Result<Response, ContractError> {

    let key = contract_addr.as_bytes();
    if let Ok(Some(_)) = MARKETS.may_load(deps.storage, &key) {
        return Err(ContractError::Std(StdError::generic_err("Market already exists")));
    }

    let market_state: VammStateResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&VammQueryMsg::VammState { })?,
    }))?;

    let config: Config = CONFIG.load(deps.storage)?;

    // Get Initial Anchor Index
    let anchor_state: AnchorStateResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: deps.api.addr_humanize(&config.anchor_addr)?.to_string(),
        msg: to_binary(&AnchorQueryMsg::State { block_height: None })?,
    }))?;

    let market = Market {
        contract_addr: deps.api.addr_canonicalize(contract_addr.as_str())?,
        cumulative_funding_premium: market_state.funding_premium_cumulative,
        cumulative_long_rewards: Decimal256::zero(),
        cumulative_short_rewards: Decimal256::zero(),
        total_long_margin: Uint256::zero(),
        total_short_margin: Uint256::zero(),
        total_margin: Uint256::zero(),
        last_anchor_index: anchor_state.global_interest_index
    };

    MARKETS.save(deps.storage, key, &market)?;

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Market { market_addr} => {
            let valid_addr = deps.api.addr_validate(&market_addr.as_str())?;
            to_binary(&query_markets(deps, valid_addr)?)
        },
        QueryMsg::Position { market_addr, user_addr } => {
            let valid_market_addr = deps.api.addr_validate(&market_addr.as_str())?;
            let valid_user_addr = deps.api.addr_validate(&user_addr.as_str())?;

            to_binary(&query_position(deps, valid_market_addr, valid_user_addr)?)
        }
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        contract_addr: deps.api.addr_humanize(&config.contract_addr)?,
        owner_addr: deps.api.addr_humanize(&config.owner_addr)?,
        stable_denom: config.stable_denom,
    })
}

fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {})
}

fn query_markets(deps: Deps, market_addr: Addr) -> StdResult<MarketResponse> {
    let market = MARKETS.load(deps.storage, (&market_addr.as_bytes()))?;
    Ok(MarketResponse {
        contract_addr: deps.api.addr_humanize(&market.contract_addr)?,
        cumulative_funding_premium: market.cumulative_funding_premium
    })
}

fn query_position(deps: Deps, amm_addr: Addr, user_addr: Addr) -> StdResult<PositionResponse> {
    let position = POSITIONS.load(deps.storage, (&amm_addr.as_bytes(), &user_addr.as_bytes()))?;

    if position.direction == Direction::NOT_SET {
        return Ok(PositionResponse {
            margin: position.margin,
            margin_left: position.margin,
            openingValue: position.openingValue,
            positionSize: position.positionSize,
            direction: position.direction,
            current_value: position.openingValue,
            margin_ratio: Decimal256::from_uint256(1u128),
            pnl: 0i64,
            funding: FundingResponse { amount: Uint256::zero(), sign: Sign::Positive }
        });
    }

    let (pnl, funding_response, new_position_value, margin_adjusted) = simulate_close(deps, amm_addr, position.clone())?;

    let margin_ratio: Decimal256 = Decimal256::from_ratio(margin_adjusted, position.openingValue);

    Ok(PositionResponse {
        margin: position.margin, 
        margin_left: margin_adjusted,
        openingValue: position.openingValue,
        positionSize: position.positionSize,
        direction: position.direction,
        current_value: new_position_value,
        margin_ratio: margin_ratio,
        pnl: pnl,
        funding: funding_response
    })
}

