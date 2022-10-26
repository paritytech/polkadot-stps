Network: ./relay-single-node.json
Creds: config

alice: is up within 600 secs

# Initialization
alice: js-script ./utils.js with "check_pre_conditions" return is 0 within 600 secs

# Sending the extrinsics
alice: js-script ./utils.js with "send_balance_transfers,1" return is 0 within 600 secs

# Calculate TPS.
alice: js-script ./utils.js with "calculate_tps,1" return is 0 within 600 secs

