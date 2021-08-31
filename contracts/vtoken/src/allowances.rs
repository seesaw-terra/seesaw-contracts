use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Storage, SubMsg, Uint128, attr};
use cw20::{AllowanceResponse, Cw20ReceiveMsg, Expiration};

use crate::error::ContractError;
use crate::state::{ALLOWANCES, BALANCES, TOKEN_INFO};

pub fn execute_increase_allowance(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    spender: String,
    amount: Uint128,
    expires: Option<Expiration>,
) -> Result<Response, ContractError> {
    let spender_addr = deps.api.addr_validate(&spender)?;
    if spender_addr == info.sender {
        return Err(ContractError::CannotSetOwnAccount {});
    }

    ALLOWANCES.update(
        deps.storage,
        (
            deps.api
                .addr_canonicalize(&info.sender.to_string())?
                .as_slice(),
            deps.api
                .addr_canonicalize(&spender_addr.to_string())?
                .as_slice(),
        ),
        |allow| -> StdResult<_> {
            let mut val = allow.unwrap_or_default();
            if let Some(exp) = expires {
                val.expires = exp;
            }
            val.allowance += amount;
            Ok(val)
        },
    )?;

    let mut messages: Vec<CosmosMsg> = vec![];

    let res = Response::new().add_messages(messages).add_attributes(vec![
        ("action", "increase_allowance"),
        ("owner", info.sender.as_str()),
        ("spender", spender.as_str()),
        ("amount", amount.to_string().as_str()),
    ]);

    Ok(res)
}

pub fn execute_decrease_allowance(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    spender: String,
    amount: Uint128,
    expires: Option<Expiration>,
) -> Result<Response, ContractError> {
    let spender_addr = deps.api.addr_validate(&spender)?;
    if spender_addr == info.sender {
        return Err(ContractError::CannotSetOwnAccount {});
    }

    let sender_raw = deps.api.addr_canonicalize(info.sender.as_str())?;
    let spender_raw = deps.api.addr_canonicalize(spender.as_str())?;

    let key = (sender_raw.as_slice(), spender_raw.as_slice());
    // load value and delete if it hits 0, or update otherwise
    let mut allowance = ALLOWANCES.load(deps.storage, key)?;
    if amount < allowance.allowance {
        // update the new amount
        allowance.allowance = allowance
            .allowance
            .checked_sub(amount)
            .map_err(StdError::overflow)?;
        if let Some(exp) = expires {
            allowance.expires = exp;
        }
        ALLOWANCES.save(deps.storage, key, &allowance)?;
    } else {
        ALLOWANCES.remove(deps.storage, key);
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    let res = Response::new().add_messages(messages).add_attributes(vec![
        ("action", "decrease_allowance"),
        ("owner", info.sender.as_str()),
        ("spender", spender.as_str()),
        ("amount", amount.to_string().as_str()),
    ]);

    Ok(res)
}

// this can be used to update a lower allowance - call bucket.update with proper keys
pub fn deduct_allowance(
    storage: &mut dyn Storage,
    api: &dyn Api,
    owner: &Addr,
    spender: &Addr,
    block: &BlockInfo,
    amount: Uint128,
) -> Result<AllowanceResponse, ContractError> {
    ALLOWANCES.update(
        storage,
        (
            api.addr_canonicalize(&owner.to_string())?.as_slice(),
            api.addr_canonicalize(&spender.to_string())?.as_slice(),
        ),
        |current| {
            match current {
                Some(mut a) => {
                    if a.expires.is_expired(block) {
                        Err(ContractError::Expired {})
                    } else {
                        // deduct the allowance if enough
                        a.allowance = a
                            .allowance
                            .checked_sub(amount)
                            .map_err(StdError::overflow)?;
                        Ok(a)
                    }
                }
                None => Err(ContractError::NoAllowance {}),
            }
        },
    )
}

pub fn execute_transfer_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    let owner_addr = deps.api.addr_validate(&owner)?;

    // deduct allowance before doing anything else have enough allowance
    deduct_allowance(
        deps.storage,
        deps.api,
        &owner_addr,
        &info.sender,
        &env.block,
        amount,
    )?;

    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&owner_addr.to_string())?
            .as_slice(),
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&rcpt_addr.to_string())?
            .as_slice(),
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let mut messages: Vec<CosmosMsg> = vec![];

    let res = Response::new().add_messages(messages).add_attributes(vec![
        ("action", "transfer_from"),
        ("from", owner.as_str()),
        ("to", recipient.as_str()),
        ("by", info.sender.as_str()),
        ("amount", amount.to_string().as_str())
    ]);
    
    Ok(res)
}

pub fn execute_burn_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let owner_addr = deps.api.addr_validate(&owner)?;

    // deduct allowance before doing anything else have enough allowance
    deduct_allowance(
        deps.storage,
        deps.api,
        &owner_addr,
        &info.sender,
        &env.block,
        amount,
    )?;

    // lower balance
    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&owner_addr.to_string())?
            .as_slice(),
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    // reduce total_supply
    TOKEN_INFO.update(deps.storage, |mut meta| -> StdResult<_> {
        meta.total_supply = meta.total_supply.checked_sub(amount)?;
        Ok(meta)
    })?;

    let mut messages: Vec<CosmosMsg> = vec![];

    let res = Response::new().add_messages(messages).add_attributes(vec![
        ("action", "burn_from"),
        ("from", owner.as_str()),
        ("by", info.sender.as_str()),
        ("amount", amount.to_string().as_str())
    ]);
    
    Ok(res)
}

pub fn execute_send_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let rcpt_addr = deps.api.addr_validate(&contract)?;
    let owner_addr = deps.api.addr_validate(&owner)?;

    // deduct allowance before doing anything else have enough allowance
    deduct_allowance(
        deps.storage,
        deps.api,
        &owner_addr,
        &info.sender,
        &env.block,
        amount,
    )?;

    // move the tokens to the contract
    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&owner_addr.to_string())?
            .as_slice(),
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        deps.api
            .addr_canonicalize(&rcpt_addr.to_string())?
            .as_slice(),
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    // create a send message
    let msg = Cw20ReceiveMsg {
        sender: info.sender.clone().into(),
        amount,
        msg,
    }
    .into_cosmos_msg(contract.clone())?;

    let mut messages: Vec<CosmosMsg> = vec![];

    messages.push(msg);

    let res = Response::new().add_messages(messages).add_attributes(vec![
        ("action", "send_from"),
        ("from", owner.as_str()),
        ("to", contract.as_str()),
        ("by", info.sender.as_str()),
        ("amount", amount.to_string().as_str())
    ]);
    
    Ok(res)
}

pub fn query_allowance(deps: Deps, owner: String, spender: String) -> StdResult<AllowanceResponse> {
    let owner_addr = deps.api.addr_validate(&owner)?;
    let spender_addr = deps.api.addr_validate(&spender)?;
    let allowance = ALLOWANCES
        .may_load(
            deps.storage,
            (
                deps.api.addr_canonicalize(owner_addr.as_str())?.as_slice(),
                deps.api
                    .addr_canonicalize(spender_addr.as_str())?
                    .as_slice(),
            ),
        )?
        .unwrap_or_default();
    Ok(allowance)
}

#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, CosmosMsg, Timestamp, WasmMsg};
    use cw20::{Cw20Coin, TokenInfoResponse};

    use crate::contract::{execute, instantiate, query_balance, query_token_info};
    use crate::msg::{ExecuteMsg, InstantiateMsg};

    fn get_balance<T: Into<String>>(deps: Deps, address: T) -> Uint128 {
        query_balance(deps, address.into()).unwrap().balance
    }

    // this will set up the instantiation for other tests
    fn do_instantiate<T: Into<String>>(
        mut deps: DepsMut,
        addr: T,
        amount: Uint128,
    ) -> TokenInfoResponse {
        let instantiate_msg = InstantiateMsg {
            name: "Auto Gen".to_string(),
            symbol: "AUTO".to_string(),
            decimals: 3,
            initial_balances: vec![Cw20Coin {
                address: addr.into(),
                amount,
            }],
            mint: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        instantiate(deps.branch(), env, info, instantiate_msg).unwrap();
        query_token_info(deps.as_ref()).unwrap()
    }

    #[test]
    fn increase_decrease_allowances() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let owner = String::from("addr0001");
        let spender = String::from("addr0002");
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        do_instantiate(deps.as_mut(), owner.clone(), Uint128::from(12340000u128));

        // no allowance to start
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        assert_eq!(allowance, AllowanceResponse::default());

        // set allowance with height expiration
        let allow1 = Uint128::from(7777u128);
        let expires = Expiration::AtHeight(5432);
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: allow1,
            expires: Some(expires),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // ensure it looks good
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        assert_eq!(
            allowance,
            AllowanceResponse {
                allowance: allow1,
                expires
            }
        );

        // decrease it a bit with no expire set - stays the same
        let lower = Uint128::from(4444u128);
        let allow2 = allow1.checked_sub(lower).unwrap();
        let msg = ExecuteMsg::DecreaseAllowance {
            spender: spender.clone(),
            amount: lower,
            expires: None,
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        assert_eq!(
            allowance,
            AllowanceResponse {
                allowance: allow2,
                expires
            }
        );

        // increase it some more and override the expires
        let raise = Uint128::from(87654u128);
        let allow3 = allow2 + raise;
        let new_expire = Expiration::AtTime(Timestamp::from_seconds(8888888888));
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: raise,
            expires: Some(new_expire),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        assert_eq!(
            allowance,
            AllowanceResponse {
                allowance: allow3,
                expires: new_expire
            }
        );

        // decrease it below 0
        let msg = ExecuteMsg::DecreaseAllowance {
            spender: spender.clone(),
            amount: Uint128::from(99988647623876347u128),
            expires: None,
        };
        execute(deps.as_mut(), env, info, msg).unwrap();
        let allowance = query_allowance(deps.as_ref(), owner, spender).unwrap();
        assert_eq!(allowance, AllowanceResponse::default());
    }

    #[test]
    fn allowances_independent() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let owner = String::from("addr0001");
        let spender = String::from("addr0002");
        let spender2 = String::from("addr0003");
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        do_instantiate(deps.as_mut(), &owner, Uint128::from(12340000u128));

        // no allowance to start
        assert_eq!(
            query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap(),
            AllowanceResponse::default()
        );
        assert_eq!(
            query_allowance(deps.as_ref(), owner.clone(), spender2.clone()).unwrap(),
            AllowanceResponse::default()
        );
        assert_eq!(
            query_allowance(deps.as_ref(), spender.clone(), spender2.clone()).unwrap(),
            AllowanceResponse::default()
        );

        // set allowance with height expiration
        let allow1 = Uint128::from(7777u128);
        let expires = Expiration::AtHeight(5432);
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: allow1,
            expires: Some(expires),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // set other allowance with no expiration
        let allow2 = Uint128::from(87654u128);
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender2.clone(),
            amount: allow2,
            expires: None,
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        // check they are proper
        let expect_one = AllowanceResponse {
            allowance: allow1,
            expires,
        };
        let expect_two = AllowanceResponse {
            allowance: allow2,
            expires: Expiration::Never {},
        };
        assert_eq!(
            query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap(),
            expect_one
        );
        assert_eq!(
            query_allowance(deps.as_ref(), owner.clone(), spender2.clone()).unwrap(),
            expect_two
        );
        assert_eq!(
            query_allowance(deps.as_ref(), spender.clone(), spender2.clone()).unwrap(),
            AllowanceResponse::default()
        );

        // also allow spender -> spender2 with no interference
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let allow3 = Uint128::from(1821u128);
        let expires3 = Expiration::AtTime(Timestamp::from_seconds(3767626296));
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender2.clone(),
            amount: allow3,
            expires: Some(expires3),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();
        let expect_three = AllowanceResponse {
            allowance: allow3,
            expires: expires3,
        };
        assert_eq!(
            query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap(),
            expect_one
        );
        assert_eq!(
            query_allowance(deps.as_ref(), owner, spender2.clone()).unwrap(),
            expect_two
        );
        assert_eq!(
            query_allowance(deps.as_ref(), spender, spender2).unwrap(),
            expect_three
        );
    }

    #[test]
    fn no_self_allowance() {
        let mut deps = mock_dependencies(&coins(2, "token"));

        let owner = String::from("addr0001");
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        do_instantiate(deps.as_mut(), &owner, Uint128::from(12340000u128));

        // self-allowance
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: owner.clone(),
            amount: Uint128::from(7777u128),
            expires: None,
        };
        let err = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(err, ContractError::CannotSetOwnAccount {});

        // decrease self-allowance
        let msg = ExecuteMsg::DecreaseAllowance {
            spender: owner,
            amount: Uint128::from(7777u128),
            expires: None,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::CannotSetOwnAccount {});
    }

    #[test]
    fn transfer_from_respects_limits() {
        let mut deps = mock_dependencies(&[]);
        let owner = String::from("addr0001");
        let spender = String::from("addr0002");
        let rcpt = String::from("addr0003");

        let start = Uint128::from(999999u128);
        do_instantiate(deps.as_mut(), &owner, start);

        // provide an allowance
        let allow1 = Uint128::from(77777u128);
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: allow1,
            expires: None,
        };
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        execute(deps.as_mut(), env, info, msg).unwrap();

        // valid transfer of part of the allowance
        let transfer = Uint128::from(44444u128);
        let msg = ExecuteMsg::TransferFrom {
            owner: owner.clone(),
            recipient: rcpt.clone(),
            amount: transfer,
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0], attr("action", "transfer_from"));

        // make sure money arrived
        assert_eq!(
            get_balance(deps.as_ref(), owner.clone()),
            start.checked_sub(transfer).unwrap()
        );
        assert_eq!(get_balance(deps.as_ref(), rcpt.clone()), transfer);

        // ensure it looks good
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        let expect = AllowanceResponse {
            allowance: allow1.checked_sub(transfer).unwrap(),
            expires: Expiration::Never {},
        };
        assert_eq!(expect, allowance);

        // cannot send more than the allowance
        let msg = ExecuteMsg::TransferFrom {
            owner: owner.clone(),
            recipient: rcpt.clone(),
            amount: Uint128::from(33443u128),
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // let us increase limit, but set the expiration (default env height is 12_345)
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: Uint128::from(1000u128),
            expires: Some(Expiration::AtHeight(env.block.height)),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        // we should now get the expiration error
        let msg = ExecuteMsg::TransferFrom {
            owner,
            recipient: rcpt,
            amount: Uint128::from(33443u128),
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Expired {});
    }

    #[test]
    fn burn_from_respects_limits() {
        let mut deps = mock_dependencies(&[]);
        let owner = String::from("addr0001");
        let spender = String::from("addr0002");

        let start = Uint128::from(999999u128);
        do_instantiate(deps.as_mut(), &owner, start);

        // provide an allowance
        let allow1 = Uint128::from(77777u128);
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: allow1,
            expires: None,
        };
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        execute(deps.as_mut(), env, info, msg).unwrap();

        // valid burn of part of the allowance
        let transfer = Uint128::from(44444u128);
        let msg = ExecuteMsg::BurnFrom {
            owner: owner.clone(),
            amount: transfer,
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0], attr("action", "burn_from"));

        // make sure money burnt
        assert_eq!(
            get_balance(deps.as_ref(), owner.clone()),
            start.checked_sub(transfer).unwrap()
        );

        // ensure it looks good
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        let expect = AllowanceResponse {
            allowance: allow1.checked_sub(transfer).unwrap(),
            expires: Expiration::Never {},
        };
        assert_eq!(expect, allowance);

        // cannot burn more than the allowance
        let msg = ExecuteMsg::BurnFrom {
            owner: owner.clone(),
            amount: Uint128::from(33443u128),
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // let us increase limit, but set the expiration (default env height is 12_345)
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: Uint128::from(1000u128),
            expires: Some(Expiration::AtHeight(env.block.height)),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        // we should now get the expiration error
        let msg = ExecuteMsg::BurnFrom {
            owner,
            amount: Uint128::from(33443u128),
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Expired {});
    }

    #[test]
    fn send_from_respects_limits() {
        let mut deps = mock_dependencies(&[]);
        let owner = String::from("addr0001");
        let spender = String::from("addr0002");
        let contract = String::from("cool-dex");
        let send_msg = Binary::from(r#"{"some":123}"#.as_bytes());

        let start = Uint128::from(999999u128);
        do_instantiate(deps.as_mut(), &owner, start);

        // provide an allowance
        let allow1 = Uint128::from(77777u128);
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: allow1,
            expires: None,
        };
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        execute(deps.as_mut(), env, info, msg).unwrap();

        // valid send of part of the allowance
        let transfer = Uint128::from(44444u128);
        let msg = ExecuteMsg::SendFrom {
            owner: owner.clone(),
            amount: transfer,
            contract: contract.clone(),
            msg: send_msg.clone(),
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0], attr("action", "send_from"));
        assert_eq!(1, res.messages.len());

        // we record this as sent by the one who requested, not the one who was paying
        let binary_msg = Cw20ReceiveMsg {
            sender: spender.clone(),
            amount: transfer,
            msg: send_msg.clone(),
        }
        .into_binary()
        .unwrap();
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract.clone(),
                msg: binary_msg,
                funds: vec![],
            }))
        );

        // make sure money sent
        assert_eq!(
            get_balance(deps.as_ref(), owner.clone()),
            start.checked_sub(transfer).unwrap()
        );
        assert_eq!(get_balance(deps.as_ref(), contract.clone()), transfer);

        // ensure it looks good
        let allowance = query_allowance(deps.as_ref(), owner.clone(), spender.clone()).unwrap();
        let expect = AllowanceResponse {
            allowance: allow1.checked_sub(transfer).unwrap(),
            expires: Expiration::Never {},
        };
        assert_eq!(expect, allowance);

        // cannot send more than the allowance
        let msg = ExecuteMsg::SendFrom {
            owner: owner.clone(),
            amount: Uint128::from(33443u128),
            contract: contract.clone(),
            msg: send_msg.clone(),
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // let us increase limit, but set the expiration to current block (expired)
        let info = mock_info(owner.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::IncreaseAllowance {
            spender: spender.clone(),
            amount: Uint128::from(1000u128),
            expires: Some(Expiration::AtHeight(env.block.height)),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        // we should now get the expiration error
        let msg = ExecuteMsg::SendFrom {
            owner,
            amount: Uint128::from(33443u128),
            contract,
            msg: send_msg,
        };
        let info = mock_info(spender.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Expired {});
    }
}
