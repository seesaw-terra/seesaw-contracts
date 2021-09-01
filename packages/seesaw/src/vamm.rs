use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr};
use cosmwasm_bignumber::{Decimal256,Uint256};
use cw20::Cw20ReceiveMsg;
use terraswap::asset::{AssetInfo, Asset};

use crate::bank::{ Direction };

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub stable_denom: String,
    pub bank_addr: Addr,
    pub init_base_reserve: Uint256,
    pub init_quote_reserve: Uint256
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    SwapIn { direction: Direction, quote_asset_amount: Uint256 }, // Used to open positions
    SwapOut { direction: Direction, quote_asset_amount: Uint256 } // Used to close position
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    BaseFromQuote { quoteAmount: Uint256, direction: Direction }, // base price from quote price
    QuoteFromBase { baseAmount: Uint256, direction: Direction }, // Base amount to Long quote amount
    OraclePrice {},
    MarketPrice {}, // Price of assets in market
    State {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub contract_addr: Addr,
    pub bank_addr: Addr,
    pub stable_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub base_asset_reserve: Uint256,
    pub quote_asset_reserve: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub base_asset_reserve: Uint256,
    pub quote_asset_reserve: Uint256,
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