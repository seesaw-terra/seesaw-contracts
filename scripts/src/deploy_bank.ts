import { mainWallet, init, upload, execute } from './utils';

(async () => {
    const bank_codeId = await upload(mainWallet,'../../artifacts/bank.wasm')
    await delay(1000)

    const bank_addr = await init(mainWallet, bank_codeId, { 
        stable_denom: 'uusd'
    }, true)
    console.log("Initialized Bank: " + bank_addr)

    console.log(typeof(bank_addr));

    const vamm_codeId = await upload(mainWallet,'../../artifacts/vamm.wasm')
    await delay(1000)

    const vamm_addr = await init(mainWallet, vamm_codeId, { 
        stable_denom: 'uusd',
        bank_addr: bank_addr,
        init_base_reserve: '1000000',
        init_quote_reserve: '1000'
    }, true)

    console.log("Initialized Vamm: " + vamm_addr)

    const res = await execute(mainWallet, bank_addr, {
        register_market: {
            contract_addr: vamm_addr
        }
    })

    console.log(res)

})()

function delay(ms: number) {
    return new Promise( resolve => setTimeout(resolve, ms, {}) );
}