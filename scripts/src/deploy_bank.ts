import { mainWallet, init, upload, execute } from './utils';

(async () => {
    const bank_codeId = await upload(mainWallet,'../../artifacts/bank.wasm')
    await delay(1000)

    const bank_addr = await init(mainWallet, bank_codeId, { 
        stable_denom: 'uusd',
        liquidation_reward: '0.05',
        liquidation_ratio: '0.0625'
    }, true)

    console.log(typeof(bank_addr));

    const vamm_codeId = await upload(mainWallet,'../../artifacts/vamm.wasm')
    await delay(1000)

    const vamm_addr = await init(mainWallet, vamm_codeId, { 
        stable_denom: 'uusd',
        bank_addr: bank_addr,
        init_base_reserve: '1000000000',
        init_quote_reserve: '32300000000'
    }, true)

    const res = await execute(mainWallet, bank_addr, {
        register_market: {
            contract_addr: vamm_addr
        }
    })

    console.log("Initialized Bank: " + bank_addr)
    console.log("Initialized Vamm: " + vamm_addr)

})()

function delay(ms: number) {
    return new Promise( resolve => setTimeout(resolve, ms, {}) );
}