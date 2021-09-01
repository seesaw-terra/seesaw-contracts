use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Uint128};
use cosmwasm_bignumber::{Decimal256,Uint256};
use cw20::Cw20ReceiveMsg;
use terraswap::asset::{AssetInfo, Asset};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    NOT_SET,
    SHORT,
    LONG
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub stable_denom: String
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    DepositStable {
        market_addr: String
    },
    RegisterMarket { // Register vAMM
        contract_addr: String
    },
    OpenPosition {
        market_addr: String,
        open_value: Uint256,
        direction: Direction
    },
    ClosePosition {
        market_addr: String
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    Config {},
    State {},
    Markets {},
    Position {
        market_addr: Addr,
        user_addr: Addr
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    WithdrawStable {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub contract_addr: Addr,
    pub owner_addr: Addr,
    pub stable_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub last_cumulative_funding_fee: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MarketsResponse {
    pub markets: Vec<MarketItem>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BorrowRateResponse {
    pub rate: Decimal256
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub margin: Uint256,
    pub openingValue: Uint256,
    pub positionSize: Uint256,
    pub direction: Direction
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MarketItem {
    pub contract_addr: Addr
}