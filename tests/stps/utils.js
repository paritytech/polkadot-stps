const { spawn, ChildProcess } = require('child_process');

async function run(nodeName, networkInfo, jsArgs) {
    const {wsUri, userDefinedTypes} = networkInfo.nodesByName[nodeName];
    const cmd = jsArgs[0];

    return new Promise((resolve, _reject) => {
        const { spawn, ChildProcess } = require('child_process');

        let cargoArgs;
        switch(cmd) {
            case "check_pre_conditions":
                const preConditionsAccountsN = jsArgs[1];
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'check-pre-conditions', '--node', wsUri, '-n', preConditionsAccountsN];
                break;
            case "send_balance_transfers":
                const balanceTransfersN = jsArgs[1];
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'send-balance-transfers', '--funded-accounts', 'tests/examples/funded-accounts.json', '--node', wsUri, '--extrinsics', balanceTransfersN];
                break;
            case "calculate_tps":
                const tpsN = jsArgs[1];
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'calculate-tps', '--node', wsUri, '-n', tpsN];
                break;
            default:
                throw new Error();
        }

        const ls = spawn('cargo', cargoArgs);

        ls.stdout.on('data', (data) => {
            process.stdout.write(data);
        });

        ls.stderr.on('data', (data) => {
            process.stdout.write(data);
        });

        ls.on('close', (code) => {
            console.log(`rust process exited with code ${code}`);
        });
        ls.on('exit', resolve);
    });

    return 0;
}

module.exports = { run }