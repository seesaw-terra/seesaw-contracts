use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Uint128};
use cosmwasm_bignumber::{ Decimal256, Uint256 };
use cw20::Cw20ReceiveMsg;
use terraswap::asset::{AssetInfo, Asset};

use crate::bank::{ Direction };

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub stable_denom: String,
    pub bank_addr: String,
    pub init_base_reserve: Uint128,
    pub init_quote_reserve: Uint128
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    SwapIn { direction: Direction, quote_asset_amount: Uint256 }, // Used to open positions
    SwapOut { direction: Direction, base_asset_amount: Uint256 }, // Used to close position
    SettleFunding {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    SimulateIn { quoteAmount: Uint256, direction: Direction }, // base price from quote price
    SimulateOut { baseAmount: Uint256, direction: Direction }, // Base amount to Long quote amount
    OraclePrice {},
    MarketPrice {}, // Price of assets in market
    State {},
    MarketInfo {},
    MarketSnapshots {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConfigResponse {
    pub contract_addr: Addr,
    pub bank_addr: Addr,
    pub stable_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WhoPays {
    LONG,
    SHORT
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Funding {
    pub amount: Decimal256,
    pub who_pays: WhoPays
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StateResponse {
    pub base_asset_reserve: Uint256,
    pub quote_asset_reserve: Uint256,
    pub funding_premium_cumulative: Decimal256,
    pub funding_fee: Funding,
    pub market_price: Decimal256,
    pub underlying_price: Decimal256
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MarketsResponse {
    pub markets: Vec<MarketItem>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct BorrowRateResponse {
    pub rate: Decimal256
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PositionResponse {
    pub margin: Uint256,
    pub openingValue: Uint256,
    pub positionSize: Uint256,
    pub direction: Direction
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MarketItem {
    pub contract_addr: Addr
}