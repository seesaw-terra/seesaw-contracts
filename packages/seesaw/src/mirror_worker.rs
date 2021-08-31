use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr};
use cosmwasm_bignumber::{Decimal256, Uint256};
use terraswap::asset::{Asset};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub terraswap_pair_addr: Addr,
    pub terraswap_lp_addr: Addr,
    pub token_addr: Addr,
    pub mirror_stake_addr: Addr,
    pub mirror_oracle_addr: Addr,
    pub owner_addr: Addr,
    pub bank_addr: Addr,
    pub stable_denom: String
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Farm {
        position_id: u64,
        borrower: Addr,
        collateral: Option<Asset>,
        loan: Uint256,
        pair_balance: Uint256
    },
    BondHook {
        prev_lp_balance: Uint256,
        position_id: u64,
        token_addr: Addr,
        terraswap_lp_addr: Addr,
        mirror_stake_addr: Addr,
        borrower: Addr
    },
    Withdraw {
        position_id: u64,
        loan: Uint256
    },
    WithdrawHook {
        position_id: u64,
        loan: Uint256,
        prev_token: Uint256,
        prev_stable: Uint256
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConfigResponse {
    pub contract_addr: Addr,
    pub terraswap_pair_addr: Addr,
    pub terraswap_lp_addr: Addr,
    pub token_addr: Addr,
    pub mirror_stake_addr: Addr,
    pub owner_addr: Addr,
    pub bank_addr: Addr,
    pub stable_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StateResponse {
    pub total_liabilities: Decimal256,
    pub total_lp_share: Decimal256
}