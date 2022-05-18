// Rust bindings that execute the `sender` binary.
async function main(url, num_ext) {
	return new Promise((resolve, _reject) => {
		const { spawn, ChildProcess } = require('child_process');
		console.log(`Compiling rust...`);
		const ls = spawn('cargo', ['r', '--quiet', '--release', '--manifest-path', 'utils/Cargo.toml', '--', 'send-balance-transfers', '--funded-accounts', 'tests/examples/funded-accounts.json', '--node', url, '--extrinsics', num_ext]);

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
}

module.exports = { main };
