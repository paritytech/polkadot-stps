
const { run: pre_condition } = require("./pre_condition");
const { run: transfer } = require("./transfer_keep_alive");
const { run: post_condition } = require("./post_condition");
const { run: tsp_meter } = require("./tps_meter");

// Entry point for local substrate development.
//
// Example:
// - Start a local substrate node with `./target/release/substrate --dev`
// - Run this script with `node standalone.js`
(async function main() {
	const args = { nodesByName: { "local": { wsUri: "ws://127.0.0.1:9944", userDefinedTypes: null } } };

	await pre_condition("local", args, null);
	await transfer("local", args, "0");
	await post_condition("local", args, null);
	await tsp_meter("local", args, null);
	console.info("JS Done");
	process.exit(0);
})();
