# This tests the complete process of the sTPS benchmark.
# It is currently run on just one node for brevity.

Network: ./relay.json
Creds: config

alice: is up within 600 secs

# Initialization
alice: js-script ./utils.js with "check_pre_conditions" return is 0 within 600 secs

# Sending the extrinsics
alice: js-script ./utils.js with "send_balance_transfers" return is 0 within 600 secs

# Calculate TPS.
# TODO return the TPS and assert its value.
alice: js-script ./utils.js with "calculate_tps" return is 0 within 600 secs

