use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_bignumber::{Decimal256, Uint256};
use terraswap::asset::{AssetInfoRaw};
use cosmwasm_std::{Api, CanonicalAddr, Decimal, Order, StdResult, Storage};
use cw_storage_plus::{Item,Map};

use seesaw::{bank::{Direction}, vamm::Funding};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum OracleType {
    NATIVE // Right now only native oracle implemented, will add Band and Mirror oracles in future.
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub contract_addr: CanonicalAddr,
    pub bank_addr: CanonicalAddr,
    pub stable_denom: String, // i.e. Quote denom
    pub oracle_type: OracleType,
    pub base_denom: String, // Optional, required if OracleType = Native
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub quote_asset_reserve: Uint256,
    pub base_asset_reserve: Uint256,
    pub funding_period: Uint256,
    pub aggregated_funding: Decimal256,
    pub funding_rate: Funding
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const STATE: Item<State> = Item::new("state");