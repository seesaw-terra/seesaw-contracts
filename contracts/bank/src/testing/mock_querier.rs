use cosmwasm_bignumber::{Decimal256, Uint256};
use schemars::JsonSchema;
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Coin, ContractResult, Decimal, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, Uint128, WasmQuery,
};
use serde::{Deserialize, Serialize};
use seesaw::bank::Direction;
use std::collections::HashMap;
use std::any::type_name;

use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg, TokenInfoResponse};
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraQueryWrapper, TerraRoute};
use seesaw::vamm::{Funding, QueryMsg as VammQueryMsg, StateResponse as VammStateResponse, WhoPays};
use moneymarket::market::{QueryMsg as AnchorQueryMsg, StateResponse as AnchorStateResponse, ConfigResponse as AnchorConfigResponse};

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(MOCK_CONTRACT_ADDR, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    token_querier: TokenQuerier,
    tax_querier: TaxQuerier,
}

#[derive(Clone, Default)]
pub struct TokenQuerier {
    // this lets us iterate over all pairs that match the first string
    balances: HashMap<String, HashMap<String, Uint128>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    VammState {},
    SimulateIn { quoteAmount: Uint256, direction: Direction },
    SimulateOut { baseAmount: Uint256, direction: Direction },
    // State {}, 
    State { block_height: Option<u64> }

}

impl TokenQuerier {
    pub fn new(balances: &[(&String, &[(&String, &Uint128)])]) -> Self {
        TokenQuerier {
            balances: balances_to_map(balances),
        }
    }
}

pub(crate) fn balances_to_map(
    balances: &[(&String, &[(&String, &Uint128)])],
) -> HashMap<String, HashMap<String, Uint128>> {
    let mut balances_map: HashMap<String, HashMap<String, Uint128>> = HashMap::new();
    for (contract_addr, balances) in balances.iter() {
        let mut contract_balances_map: HashMap<String, Uint128> = HashMap::new();
        for (addr, balance) in balances.iter() {
            contract_balances_map.insert(addr.to_string(), **balance);
        }

        balances_map.insert(contract_addr.to_string(), contract_balances_map);
    }
    balances_map
}

#[derive(Clone, Default)]
pub struct TaxQuerier {
    rate: Decimal,
    // this lets us iterate over all pairs that match the first string
    caps: HashMap<String, Uint128>,
}

impl TaxQuerier {
    pub fn new(rate: Decimal, caps: &[(&String, &Uint128)]) -> Self {
        TaxQuerier {
            rate,
            caps: caps_to_map(caps),
        }
    }
}

pub(crate) fn caps_to_map(caps: &[(&String, &Uint128)]) -> HashMap<String, Uint128> {
    let mut owner_map: HashMap<String, Uint128> = HashMap::new();
    for (denom, cap) in caps.iter() {
        owner_map.insert(denom.to_string(), **cap);
    }
    owner_map
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.handle_query(&request)
    }
}

fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}


impl WasmMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match &request {
            QueryRequest::Custom(TerraQueryWrapper { route, query_data }) => {
                if route == &TerraRoute::Treasury {
                    match query_data {
                        TerraQuery::TaxRate {} => {
                            let res = TaxRateResponse {
                                rate: self.tax_querier.rate,
                            };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        TerraQuery::TaxCap { denom } => {
                            let cap = self
                                .tax_querier
                                .caps
                                .get(denom)
                                .copied()
                                .unwrap_or_default();
                            let res = TaxCapResponse { cap };
                            SystemResult::Ok(ContractResult::from(to_binary(&res)))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                    }
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                match from_binary(&msg).unwrap() {
                        QueryMsg::VammState {} => {
                            SystemResult::Ok(ContractResult::Ok(
                                to_binary(&VammStateResponse {
                                    base_asset_reserve: Uint256::from(1000u128),
                                    quote_asset_reserve: Uint256::from(1_000_000u128),
                                    funding_premium_cumulative: Decimal256::from_uint256(10_000u128),
                                    funding_fee: Funding {
                                        amount: Decimal256::from_ratio(1, 1000),
                                        who_pays: WhoPays::LONG
                                    },
                                    market_price: Decimal256::from_uint256(1100u128),
                                    underlying_price: Decimal256::from_uint256(1000u128)
                                })
                                .unwrap(),
                            ))
                        },
                        QueryMsg::Config {} => {
                            SystemResult::Ok(ContractResult::Ok(
                                to_binary(&AnchorConfigResponse {
                                    owner_addr: "owner".to_string(),
                                    aterra_contract: "aterra".to_string(),
                                    interest_model: "interest".to_string(),
                                    distribution_model: "dist".to_string(),
                                    overseer_contract: "over".to_string(),
                                    collector_contract: "collector".to_string(),
                                    distributor_contract: "dist".to_string(),
                                    stable_denom: "uusd".to_string(),
                                    max_borrow_factor: Decimal256::one(),
                                }).unwrap()
                            ))
                        }
                        QueryMsg::SimulateIn { quoteAmount, direction} => {
                            SystemResult::Ok(ContractResult::Ok(
                                to_binary(&(quoteAmount / Decimal256::from_uint256(10u128))
                            ).unwrap(),
                            ))
                        },
                        QueryMsg::SimulateOut { baseAmount, direction} => {
                            SystemResult::Ok(ContractResult::Ok(
                                to_binary(&(baseAmount * Uint256::from(9u128))
                            ).unwrap(),
                            ))
                        },
                        QueryMsg::State { block_height } => {
                            SystemResult::Ok(ContractResult::Ok(
                                to_binary(&AnchorStateResponse {
                                    total_liabilities: Decimal256::zero(),
                                    total_reserves: Decimal256::zero(),
                                    last_interest_updated: 1u64,
                                    last_reward_updated: 0u64,
                                    global_interest_index: Decimal256::one(),
                                    global_reward_index: Decimal256::zero(),
                                    anc_emission_rate: Decimal256::zero(),
                                    prev_aterra_supply: Uint256::zero(),
                                    prev_exchange_rate: Decimal256::zero(),
                                }).unwrap()
                            ))
                        }
                        _ => panic!("DO NOT ENTER HERE"),
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<TerraQueryWrapper>) -> Self {
        WasmMockQuerier {
            base,
            token_querier: TokenQuerier::default(),
            tax_querier: TaxQuerier::default(),
        }
    }

    // configure the mint whitelist mock querier
    pub fn with_token_balances(&mut self, balances: &[(&String, &[(&String, &Uint128)])]) {
        self.token_querier = TokenQuerier::new(balances);
    }

    // configure the token owner mock querier
    pub fn with_tax(&mut self, rate: Decimal, caps: &[(&String, &Uint128)]) {
        self.tax_querier = TaxQuerier::new(rate, caps);
    }

    pub fn with_balance(&mut self, balances: &[(&String, Vec<Coin>)]) {
        for (addr, balance) in balances {
            self.base.update_balance(addr.to_string(), balance.clone());
        }
    }
}