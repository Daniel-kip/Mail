# Web UI / learning API: private network only (never publish port 11334).
bind_socket = "*:11334";
.include(try=true; priority=10) "$LOCAL_CONFDIR/local.d/controller-password.inc"
