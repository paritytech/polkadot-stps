Network: ./solo.toml
Creds: config


alice: is up
bob: is up
charlie: is up

# Initialization
alice: js-script ./pre_condition.js within 60 secs
bob: js-script ./pre_condition.js within 60 secs
charlie: js-script ./pre_condition.js within 60 secs

# Sending the extrinsics
alice: js-script ./transfer_keep_alive.js with "0" within 240 secs

# Verification
alice: js-script ./post_condition.js within 60 secs
bob: js-script ./post_condition.js within 60 secs
charlie: js-script ./post_condition.js within 60 secs
