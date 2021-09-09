import * as path from "path";
import BN from "bn.js";
import chalk from "chalk";
import * as chai from "chai";
import chaiAsPromised from "chai-as-promised";
import { LocalTerra, MsgExecuteContract, Coins, Coin } from "@terra-money/terra.js";
import {
  toEncodedBinary,
  sendTransaction,
  storeCode,
  instantiateContract,
  queryNativeTokenBalance,
  queryTokenBalance,
} from "./helpers";
import { mainWallet, init, upload, execute, terra, query} from './utils';

chai.use(chaiAsPromised);
const { expect } = chai;

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

let bankAddr: string = 'terra1x2lsmwv6ljrm9lh6j6z0rrh263rz96vppd9wvf';
let vammAddr: string = 'terra1jjn9sm4caslf66g96t0e5zm5vj7upwp8xre8u4';
let walletAddr: string = 'terra1gfu9uymnr04amjtssfamzymuwna303awyz9kch';

let anchorAddr: string = 'terra15dwd5mj8v59wpj0wvt233mf5efdff808c5tkal';
//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function testAddMargin() {

  const res = await execute(mainWallet, bankAddr, {
    deposit_stable: {
      market_addr: vammAddr
    }},'10000000uusd' // UST value of margin
  )

  const poolUUsd = await queryNativeTokenBalance(terra, bankAddr, "uusd");

  const position_res = await query(bankAddr, {
    position: {
      market_addr: vammAddr,
      user_addr: walletAddr
    }
    }
  )
  console.log(position_res)
  
  console.log(chalk.green("Passed!"));
}

async function testOpenPosition() {
  const open_res = await execute(mainWallet, bankAddr, {
    open_position: {
      market_addr: vammAddr,
      open_value: "20000000", //UST value of position
      direction: "l_o_n_g"
    }}
  )
  console.log(open_res)

  const position_res = await query(bankAddr, {
    position: {
      market_addr: vammAddr,
      user_addr: walletAddr
    }
    }
  )
  console.log(position_res)

}

async function testClosePosition() {
  const open_res = await execute(mainWallet, bankAddr, {
    close_position: {
      market_addr: vammAddr
    }}
  )
  console.log(open_res)

  const position_res = await query(bankAddr, {
    position: {
      market_addr: vammAddr,
      user_addr: walletAddr
    }
    }
  )
  console.log(position_res)

}


async function queryState() {

  const state_res = await query(vammAddr, {
    state: {}
    }
  )
  console.log(state_res)

}

async function querySnapshots() {

  const state_res = await query(vammAddr, {
    market_snapshots: {}
    }
  )
  console.log(state_res)

}

async function queryPosition() {

  const position_res = await query(bankAddr, {
    position: {
      market_addr: vammAddr,
      user_addr: walletAddr
    }
    }
  )
  console.log(position_res)

}


async function settleFunding() {

  const position_res = await execute(mainWallet, bankAddr, {
    update_funding: {
      market_addr: vammAddr,
    }
    }
  )
  console.log(position_res)

}


async function queryAnchor() {
  const res = await query(anchorAddr, {
    state: {
      block_height: null
    }
  })
  console.log(res);
}



async function main() {
  // await testAddMargin();
  // settleFunding();
  // await testOpenPosition();
  await testClosePosition();
  // await querySnapshots();
  // await queryState();
  // await queryPosition();
  // await queryAnchor();
}

main()

//----------------------------------------------------------------------------------------
// Test 2. Swap
//
// User 2 sells 1 MIR for UST
//
// k = poolUMir * poolUUsd
// = 69000000 * 420000000 = 28980000000000000
// returnAmount = poolUusd - k / (poolUMir + offerUMir)
// = 420000000 - 28980000000000000 / (69000000 + 1000000)
// = 6000000
// fee = returnAmount * feeRate
// = 6000000 * 0.003
// = 18000
// returnAmountAfterFee = returnUstAmount - fee
// = 6000000 - 18000
// = 5982000
// returnAmountAfterFeeAndTax = deductTax(5982000) = 5976023
// transaction cost for pool = addTax(5976023) = 5981999
//
// Result
// ---
// pool uMIR  69000000 + 1000000 = 70000000
// pool uusd  420000000 - 5981999 = 414018001
// user uLP   170235131
// user uMIR  10000000000 - 1000000 = 9999000000
// user uusd  balanceBeforeSwap + 5976023 - 4500000 (gas)
//----------------------------------------------------------------------------------------
