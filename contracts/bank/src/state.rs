use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_bignumber::{Decimal256, Uint256};
use terraswap::asset::{AssetInfoRaw};
use cosmwasm_std::{CanonicalAddr, Api, Storage, StdResult, Order};
use cw_storage_plus::{Item,Map};

use seesaw::bank::{Direction};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub contract_addr: CanonicalAddr,
    pub owner_addr: CanonicalAddr,
    pub anchor_addr: CanonicalAddr,
    pub stable_denom: String,
    pub liquidation_ratio: Decimal256,
    pub liquidation_reward: Decimal256
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State { }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub margin: Uint256, // Value of margin, in default denom
    pub direction: Direction, // true = longing, false = shorting
    pub openingValue: Uint256, // Amount of base asset (i.e. UST) that is used in shorting/longing, at the time of opening
    pub positionSize: Uint256, // Amount of quoted assets that is being longed/shorted
    pub last_cumulative_funding: Decimal256,
    pub last_cumulative_long_rewards: Decimal256,
    pub last_cumulative_short_rewards: Decimal256
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Market {
    pub contract_addr: CanonicalAddr,
    pub cumulative_funding_premium: Decimal256,
    pub cumulative_long_rewards: Decimal256,
    pub cumulative_short_rewards: Decimal256,
    pub total_long_margin: Uint256,
    pub total_short_margin: Uint256,
    pub total_margin: Uint256,
    pub last_anchor_index: Decimal256
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const STATE: Item<State> = Item::new("state");

pub const MARKETS: Map<&[u8], Market> = Map::new("markets");

pub const POSITIONS: Map<(&[u8], &[u8]), Position> = Map::new("position");