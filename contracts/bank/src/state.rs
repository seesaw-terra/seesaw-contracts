use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_bignumber::{Decimal256, Uint256};
use terraswap::asset::{AssetInfoRaw};
use cosmwasm_std::{CanonicalAddr, Api, Storage, StdResult, Order};
use cw_storage_plus::{Item,Map};

use seesaw::bank::{MarketItem, Direction};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub contract_addr: CanonicalAddr,
    pub owner_addr: CanonicalAddr,
    pub stable_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub last_cumulative_funding_fee: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub margin: Uint256, // Value of margin, in default denom
    pub direction: Direction, // true = longing, false = shorting
    pub openingValue: Uint256, // Amount of base asset (i.e. UST) that is used in shorting/longing, at the time of opening
    pub positionSize: Uint256 // Amount of quoted assets that is being longed/shorted
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Market {
    pub contract_addr: CanonicalAddr,
}

pub fn read_markets(
    storage: &dyn Storage,
    api: &dyn Api
) -> StdResult<Vec<MarketItem>> {
    MARKETS
    .range(storage, None, None, Order::Ascending)
    .map(|item| {
        let (_, v) = item?;
        Ok(MarketItem {
            contract_addr: api.addr_humanize(&v.contract_addr)?,
        })
    })
    .collect::<StdResult<Vec<MarketItem>>>()
}


pub fn pair_key(asset_infos: &[AssetInfoRaw; 2]) -> Vec<u8> {
    let mut asset_infos = asset_infos.to_vec();
    asset_infos.sort_by(|a, b| a.as_bytes().cmp(&b.as_bytes()));

    [asset_infos[0].as_bytes(), asset_infos[1].as_bytes()].concat()
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const STATE: Item<State> = Item::new("state");

pub const MARKETS: Map<&[u8], Market> = Map::new("markets");

pub const POSITIONS: Map<(&[u8], &[u8]), Position> = Map::new("position");