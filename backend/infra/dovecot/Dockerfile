# Dovecot (IMAP, LMTP, SASL, quota policy, Sieve/ManageSieve) for Mailvora.
FROM debian:bookworm-slim

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      dovecot-core dovecot-imapd dovecot-lmtpd dovecot-pgsql dovecot-sieve dovecot-managesieved ca-certificates gettext-base openssl \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd -g 5000 vmail && useradd -u 5000 -g vmail -d /var/vmail -s /usr/sbin/nologin vmail \
 && rm -rf /etc/dovecot/conf.d /etc/dovecot/dovecot.conf

COPY dovecot.conf.template dovecot-sql.conf.ext.template /etc/dovecot/templates/
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 0755 /usr/local/bin/entrypoint.sh

VOLUME ["/var/vmail"]
EXPOSE 24 143 993 4190 8080 10993 12345
HEALTHCHECK --interval=30s --timeout=5s CMD doveadm service status imap >/dev/null 2>&1 || exit 1
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
