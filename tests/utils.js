const { spawn, ChildProcess } = require('child_process');

async function run(nodeName, networkInfo, jsArgs) {
    const wsUri = networkInfo.nodesByName[nodeName].wsUri;
    const cmd = jsArgs[0];
    const totalSenders = jsArgs[1];
    const totalTx = jsArgs[2];
    const relayOrPara = jsArgs[3]; // rococo or polkadot-parachain, used for compilation features
    const collator = jsArgs[4]; // the collator to create the parachain-rpc client for testing parachain TPS
    const paraId = jsArgs[5]; // the parachain-id used for testing parachain TPS
    let collatorUri = networkInfo.nodesByName[collator].wsUri;
    let senderIndex = nodeName.split("-")[1];

    return new Promise((resolve, _reject) => {
        let cargoArgs;
        switch(cmd) {
            case "send_balance_transfers":
                if (senderIndex) {
                    cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/sender/Cargo.toml', '--features', relayOrPara, '--', '--node-url', wsUri, '--sender-index', senderIndex, '--total-senders', totalSenders, '--num', totalTx];
                } else {
                    cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/sender/Cargo.toml', '--features', relayOrPara, '--', '--node-url', wsUri, '--sender-index', 0, '--total-senders', totalSenders, '--num', totalTx];
                }
                break;
            case "calculate_tps":
                // The first conditional assumes that the node running the script is the validator node
                if (relayOrPara == "polkadot-parachain") {
                    cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/tps/Cargo.toml', '--features', relayOrPara, '--', '--para-finality', '--para-id', paraId, '--validator-url', wsUri, '--collator-url', collatorUri, '--num', totalTx, '--total-senders', totalSenders];
                } else {
                    cargoArgs = ['r', '--quiet', '--release', '--manifest-path', 'utils/tps/Cargo.toml', '--features', relayOrPara, '--', '--node-url', wsUri, '--num', totalTx, '--total-senders', totalSenders, '--genesis'];
                }
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