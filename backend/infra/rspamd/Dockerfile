# Rspamd spam filter (milter for Postfix).
FROM debian:bookworm-slim
RUN apt-get update  && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends rspamd ca-certificates  && rm -rf /var/lib/apt/lists/*
COPY local.d/ /etc/rspamd/local.d/
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 0755 /usr/local/bin/entrypoint.sh
EXPOSE 11332 11333 11334
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
