const { spawn, ChildProcess } = require('child_process');

async function run(nodeName, networkInfo, jsArgs) {
    const {wsUri} = networkInfo.nodesByName[nodeName];
    const cmd = jsArgs[0];

    return new Promise((resolve, _reject) => {
        const { spawn, ChildProcess } = require('child_process');

        let cargoArgs;
        switch(cmd) {
            case "check_pre_conditions":
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'check-pre-conditions', '--node', wsUri];
                break;
            case "send_balance_transfers":
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'send-balance-transfers', '--node', wsUri];
                break;
            case "calculate_tps":
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'calculate-tps', '--node', wsUri];
                break;
            default:
                throw new Error();
        }

        const p = spawn('cargo', cargoArgs);

        p.stdout.on('data', (data) => {
            process.stdout.write(data);
        });

        p.stderr.on('data', (data) => {
            process.stdout.write(data);
        });

        p.on('close', (code) => {
            console.log(`rust process exited with code ${code}`);
        });
        p.on('exit', resolve);
    });

    return 0;
}

module.exports = { run }