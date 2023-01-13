const { spawn, ChildProcess } = require('child_process');

async function run(nodeName, networkInfo, jsArgs) {
    const {wsUri} = networkInfo.nodesByName[nodeName];
    const cmd = jsArgs[0];
    let senderIndex = nodeName.split("-")[1];

    return new Promise((resolve, _reject) => {
        let cargoArgs;
        switch(cmd) {
            case "send_balance_transfers":
                const totalSenders = jsArgs[1];
                const totalTx = jsArgs[2];
                if (senderIndex) {
                    cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/sender/Cargo.toml', '--', '--node-url', wsUri, '--sender-index', senderIndex, '--total-senders', totalSenders, '-n', totalTx];
                } else {
                    cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/sender/Cargo.toml', '--', '--node-url', wsUri, '--sender-index', 0, '--total-senders', totalSenders, '-n', totalTx];
                }

                break;
            case "calculate_tps":
                const totalTx = jsArgs[1];
                cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/tps/Cargo.toml', '--', '--node-url', wsUri, '-n', totalTx];
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
        p.on('exit', (code) => {
            resolve(code);
        })
    });
}

module.exports = { run }