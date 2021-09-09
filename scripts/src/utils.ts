
import {
    LCDClient,
    MnemonicKey,
    MsgStoreCode,
    StdFee,
    MsgInstantiateContract,
    MsgExecuteContract,
    Coins,
    isTxError
  } from '@terra-money/terra.js'
  
  import * as fs from 'fs'
  
  export const network = {
      chainID: 'bombay-10',
      lcd: 'https://bombay-lcd.terra.dev',
      name: 'bombay-10',
  }
  
  export const terra = new LCDClient({
      URL: network.lcd,
      chainID: network.chainID,
      gasAdjustment: 1.15,
  });
  
  const key = new MnemonicKey({
      mnemonic: 'merge juice feel flee laptop track salad deliver bird replace pride nature oven creek neutral toward upgrade caution advance trend method aspect tooth region'
  })
  
  export const mainWallet = terra.wallet(key)
  
  export const upload = async (wallet, path, gasLimit = 10000000) => {
      const tx = await wallet.createAndSignTx({
        msgs: [
          new MsgStoreCode(
            wallet.key.accAddress,
            fs.readFileSync(path, { encoding: "base64" })
          ),
        ],
        fee: new StdFee(gasLimit, "20000000uusd"),
      });
      const result = await terra.tx.broadcast(tx);
      if (
        result.raw_log ===
        "unauthorized: signature verification failed; verify correct account sequence and chain-id"
      ) {
        return upload(wallet, path, gasLimit);
      }
      if (isTxError(result)) {
        throw new Error(
          `store code failed. code: ${result.code}, codespace: ${result.codespace}, raw_log: ${result.raw_log}`
        );
      }
      const logs = JSON.parse(result.raw_log);
      let codeId;
      for (const log of logs) {
        if (log.events) {
          for (const ev of log.events) {
            if (ev.type === "store_code") {
              codeId = ev.attributes.find((att) => att.key === "code_id").value;
              break;
            }
          }
        }
        if (codeId) {
          break;
        }
      }
      console.log("Upload", path, "Code", codeId);
      return Number(codeId);
    };
  
  export const init = async (
      wallet,
      codeId,
      msgObj,
      migratable = false,
      gasLimit = 5000000
    ) => {
      console.log('codeId', codeId)
      console.log(typeof(codeId))
      const tx = await wallet.createAndSignTx({
        msgs: [
          new MsgInstantiateContract(
            wallet.key.accAddress,
            null,
            codeId,
            msgObj,
            [],
          ),
        ],
        fee: new StdFee(gasLimit, "1000000uusd"),
      });
      const result = await terra.tx.broadcast(tx);
      if (
        result.raw_log ===
        "unauthorized: signature verification failed; verify correct account sequence and chain-id"
      ) {
        return init(wallet, codeId, msgObj, migratable, gasLimit);
      }
      console.log('result is ', result)
      if (isTxError(result)) {
        throw new Error(
          `store code failed. code: ${result.code}, codespace: ${result.codespace}, raw_log: ${result.raw_log}`
        );
      }
      let addr;
      for (const log of result.logs) {
        if (log.events) {
          for (const ev of log.events) {
            if (ev.type === "instantiate_contract") {
              addr = ev.attributes.find((att) => att.key === "contract_address").value;
              break;
            }
          }
        }
        if (addr) {
          break;
        }
      }
      console.log("Init", codeId, "at", addr);
      return addr;
    };
  
    export const execute = async (
      wallet,
      addr,
      msgObj,
      coins?: Coins.Input,
      gasLimit = 5000000,
      amount = "1000000uusd"
    ) => {
      console.log('msg construct' , msgObj)
      const tx = await wallet.createAndSignTx({
        msgs: [new MsgExecuteContract(wallet.key.accAddress, addr, msgObj, coins)],
        fee: new StdFee(gasLimit, amount),
      });
      console.log('before contract execution')
      const result = await terra.tx.broadcast(tx);
      console.log(result)
      if (isTxError(result)) {
        throw new Error(
          `store code failed. code: ${result.code}, codespace: ${result.codespace}, raw_log: ${result.raw_log}`
        );
      }
      console.log(result);
      if (
        result.raw_log ===
        "unauthorized: signature verification failed; verify correct account sequence and chain-id"
      ) {
        return execute(wallet, addr, msgObj, coins, gasLimit);
      }
      const txHash = result.txhash;
      console.log("Exec", addr, "at", txHash);
      return txHash;
    };

    export const query = async (addr,msg) => {
        const result = await terra.wasm.contractQuery(addr, msg)
        return result
      }