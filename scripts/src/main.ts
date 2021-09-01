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
import { mainWallet, init, upload, execute, terra} from './utils';

chai.use(chaiAsPromised);
const { expect } = chai;

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

let bankAddr: string = 'terra1wd02pjkgt8v7yxj6pryja4qzgmuz3fyfz9mlwk';
let vammAddr: string = 'terra1xp2366g3nkwn37yeruvt40mjdrktz7uvrwfd3n';


//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function testAddMargin() {

  const res = await execute(mainWallet, bankAddr, {
    deposit_stable: {
      market_addr: vammAddr
    }},'10uusd'
  )

  const poolUUsd = await queryNativeTokenBalance(terra, bankAddr, "uusd");
  console.log(poolUUsd)
  
  console.log(chalk.green("Passed!"));
}

testAddMargin()

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
