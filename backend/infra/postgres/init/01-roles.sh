#!/bin/sh
# Executed once by the postgres image on first start of an empty data volume.
#
# Role separation (guides 02 and 06):
#   $POSTGRES_USER  owner role; runs migrations (DDL).
#   mv_app          runtime role for the API and workers; DML only, no DDL.
#   mv_mailserver   Postfix/Dovecot lookups; SELECT on the mail_* views only
#                   (granted by migration 0003).
#
# Passwords come from the environment. The defaults exist only so the local
# development stack starts without configuration.
set -eu
APP_PW="${MV_APP_PASSWORD:-mv_app_dev}"
MAIL_PW="${MV_MAILSERVER_PASSWORD:-mv_mailserver_dev}"

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB"      -v app_pw="$APP_PW" -v mail_pw="$MAIL_PW" -v owner="$POSTGRES_USER" -v db="$POSTGRES_DB" <<'SQL'
CREATE ROLE mv_app LOGIN PASSWORD :'app_pw';
CREATE ROLE mv_mailserver LOGIN PASSWORD :'mail_pw';
GRANT CONNECT ON DATABASE :"db" TO mv_app, mv_mailserver;
GRANT USAGE ON SCHEMA public TO mv_app, mv_mailserver;
ALTER DEFAULT PRIVILEGES FOR ROLE :"owner" IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO mv_app;
ALTER DEFAULT PRIVILEGES FOR ROLE :"owner" IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO mv_app;
SQL
