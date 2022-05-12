# This tests the complete process of the sTPS benchmark.
# It is currently run on just one node for brevity.
# The number of extrinsics is set to 16384 but can be increased
# for a real results.

Network: ./relay.json
Creds: config

alice: is up within 600 secs
#bob: is up within 600 secs

# Initialization
alice: js-script ./pre_condition.js with "16384"

# The RPC sometimes hangs in the beginning, hopefully this fixes it.
alice: reports block height is at least 1 within 60 seconds

# Sending the extrinsics
alice: js-script ./transfer_keep_alive.js with "16384" within 600 secs

# Verification
alice: js-script ./wait_for_events.js with "16384" within 600 secs

# Calculate TPS.
# TODO return the TPS and assert its value.
alice: js-script ./tps_meter.js within 120 secs
