# Milter mode for Postfix (smtpd_milters = inet:rspamd:11332).
bind_socket = "*:11332";
milter = yes;
timeout = 120s;
upstream "local" {
  default = yes;
  self_scan = yes;
}
