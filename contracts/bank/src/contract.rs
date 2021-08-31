use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, CanonicalAddr, Reply, StdError, attr, SubMsg, WasmMsg, ReplyOn, from_binary, Addr
};
use cosmwasm_bignumber::{Decimal256, Uint256};
use protobuf::Message;
use terraswap::asset::{AssetInfo};
use terraswap::token::InstantiateMsg as TokenInstantiateMsg;
use cw20::{MinterResponse, Cw20ReceiveMsg};
use seesaw::bank::{BorrowRateResponse, ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MarketsResponse, PositionResponse, QueryMsg, StateResponse};

use crate::error::ContractError;
use crate::state::{CONFIG, Config, POSITIONS, Position, STATE, State, MARKETS, Market, read_markets};
use crate::response::MsgInstantiateContractResponse;
use crate::deposit::{ add_margin };

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
        stable_denom: msg.stable_denom,
    };
    CONFIG.save(deps.storage, &config)?;

    let state = State {
        last_interest_updated: env.block.height,
        total_liabilities: Decimal256::zero(),
        total_debt_share: Decimal256::zero(),
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
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env,info, msg),
        ExecuteMsg::DepositStable { market_addr } => add_margin(deps, env, info, market_addr),
        ExecuteMsg::RegisterMarket { contract_addr } => register_market(deps, env, info, contract_addr),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::WithdrawStable { }) => {
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

    let market = Market {
        contract_addr: deps.api.addr_canonicalize(contract_addr.as_str())?, 
    };

    MARKETS.save(deps.storage, key, &market)?;

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Markets {} => to_binary(&query_markets(deps)?),
        QueryMsg::Position { market_addr, user_addr } => to_binary(&query_position(deps, market_addr, user_addr)?)
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
    Ok(StateResponse {
        last_interest_updated: state.last_interest_updated,
        total_liabilities: state.total_liabilities,
    })
}

fn query_markets(deps: Deps) -> StdResult<MarketsResponse> {
    Ok(MarketsResponse {
        markets: read_markets(deps.storage, deps.api)?
    })
}

fn query_position(deps: Deps, amm_addr: Addr, user_addr: Addr) -> StdResult<PositionResponse> {
    let position = POSITIONS.load(deps.storage, (&amm_addr.as_bytes(), user_addr.to_string().as_bytes()))?;
    Ok(PositionResponse {
        margin: position.margin,
        openingValue: position.openingValue,
        positionSize: position.positionSize,
        direction: position.direction
    })
}

// #[entry_point]
// pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
//     let data = msg.result.unwrap().data.unwrap();
//     let result:MsgInstantiateContractResponse = Message::parse_from_bytes(data.as_slice()).map_err(|_| {
//         StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
//     })?;
//     let contract_addr = result.get_contract_address();

//     let api = deps.api;
//     CONFIG.update(deps.storage, |mut meta| -> StdResult<_> {
//         meta.ib_token_addr = api.addr_canonicalize(contract_addr)?;
//         Ok(meta)
//     })?;
//     Ok(Response::default())
// }