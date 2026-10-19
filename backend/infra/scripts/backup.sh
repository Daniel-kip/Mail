#!/usr/bin/env bash
# Encrypted, consistent backup of a Mailvora VPS: database, mailboxes and
# configuration, written to one dated archive and pruned after RETENTION_DAYS.
#
#   BACKUP_PASSPHRASE_FILE=/root/.mailvora-backup-key ./backup.sh /var/backups/mailvora
#
# Copy the archives off the server (rclone/rsync to another provider) so a
# compromise of this host cannot delete them. Test a restore every quarter:
# see frontend/resources/wiki/VPS-Deployment.md, "Restore".
set -euo pipefail

DEST="${1:-/var/backups/mailvora}"
STACK_DIR="${STACK_DIR:-/opt/mailvora/backend}"
RETENTION_DAYS="${RETENTION_DAYS:-14}"
KEY_FILE="${BACKUP_PASSPHRASE_FILE:?Set BACKUP_PASSPHRASE_FILE to a root-only file holding the encryption passphrase}"

[ -r "$KEY_FILE" ] || { echo "backup: cannot read $KEY_FILE" >&2; exit 1; }
[ "$(stat -c %a "$KEY_FILE")" -le 600 ] || { echo "backup: $KEY_FILE must be chmod 600" >&2; exit 1; }
command -v gpg >/dev/null || { echo "backup: gpg is required (apt install gnupg)" >&2; exit 1; }

umask 077
mkdir -p "$DEST"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
compose() { docker compose -f "$STACK_DIR/docker-compose.yml" --env-file "$STACK_DIR/stack.env" "$@"; }

echo "backup: database"
compose exec -T postgres pg_dump -U mailvora -d mailvora -Fc --no-owner > "$WORK/database.dump"

echo "backup: mailboxes"
compose run --rm --no-deps -T -v "$WORK:/out" --entrypoint sh dovecot -c 'tar -C /var/vmail -czf /out/vmail.tar.gz .'

echo "backup: configuration"
cp "$STACK_DIR/stack.env" "$WORK/stack.env"
sha256sum "$WORK"/* > "$WORK/SHA256SUMS"

ARCHIVE="$DEST/mailvora-$STAMP.tar.gpg"
tar -C "$WORK" -cf - . | gpg --batch --yes --pinentry-mode loopback --passphrase-file "$KEY_FILE" \
    --symmetric --cipher-algo AES256 --compress-algo none -o "$ARCHIVE"
echo "backup: wrote $ARCHIVE ($(du -h "$ARCHIVE" | cut -f1))"

find "$DEST" -name 'mailvora-*.tar.gpg' -mtime +"$RETENTION_DAYS" -print -delete
