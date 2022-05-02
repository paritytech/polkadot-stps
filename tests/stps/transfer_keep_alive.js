const { main: rustMain } = require("./rs_bindings");

async function run(nodeName, networkInfo, args) {
	const NUM_EXT = parseInt(args);
	const { wsUri, _userDefinedTypes } = networkInfo.nodesByName[nodeName];
	
	await rustMain(wsUri, NUM_EXT);
	console.info(`Rust done`);
}

module.exports = { run }
