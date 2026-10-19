# The private compose network (platform relay, Postfix) counts as local.
local_addrs = "127.0.0.0/8, 10.89.0.0/24";
dns {
  timeout = 2s;
  retransmits = 2;
}
