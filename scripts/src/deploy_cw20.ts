import { mainWallet, init, upload, execute } from './utils';

(async () => {
    const token_codeId = await upload(mainWallet,'../../artifacts/vtoken.wasm')
    await delay(1000)

    const token_addr = await init(mainWallet, token_codeId, { 
        name: 'mock token',
        symbol: 'MOCK',
        decimals: 6,
        initial_balances: [{
            address: 'terra1eell2f9n8j7aapz897sgc9cw3gu8apxfkdzser',
            amount: '1000000000000'
        }],
        mint: {
            minter: 'terra1eell2f9n8j7aapz897sgc9cw3gu8apxfkdzser',
            cap: '9999999999999999'
        }
    }, true)
    console.log(token_addr)

    await execute(mainWallet, token_addr, {
        increase_allowance:{
            spender: 'terra1hsdgma9vuarn6fhj9c5l80mz3tmem9t4nuawu5',
            amount: '99999999999999999999'
        }
    })

})()

function delay(ms: number) {
    return new Promise( resolve => setTimeout(resolve, ms, {}) );
}