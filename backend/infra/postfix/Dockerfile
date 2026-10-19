# Postfix MTA for Mailvora (inbound MX, client submission, platform relay).
FROM debian:bookworm-slim

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      postfix postfix-pgsql ca-certificates curl gettext-base openssl \
 && rm -rf /var/lib/apt/lists/*

COPY main.cf.template master.cf.template /etc/postfix/templates/
COPY pgsql/ /etc/postfix/templates/pgsql/
COPY bounce-to-api.sh /usr/local/bin/bounce-to-api.sh
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 0755 /usr/local/bin/bounce-to-api.sh /usr/local/bin/entrypoint.sh

EXPOSE 25 465 587 10587
HEALTHCHECK --interval=30s --timeout=5s CMD postfix status || exit 1
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
