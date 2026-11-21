#!/usr/bin/env bash
# Runs NAS mode end to end without root. A user namespace maps uids 1-65536
# from /etc/subuid, so the supervisor really switches identities and the
# kernel really enforces permissions.
#
# Needs: util-linux unshare and setpriv, newuidmap with a subuid/subgid range
# for your user, python3 with bcrypt, curl and Go.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d /tmp/boxbox-nas-e2e.XXXXXX)
trap 'rm -rf "$work"' EXIT
chmod 0755 "$work"
(cd "$here/../../backend" && go build -o "$work/server" ./cmd/server)
chmod 0755 "$work/server"
status=0
unshare --map-auto --map-root-user --setuid 0 --setgid 0 --mount \
  bash "$here/inner.sh" "$work" "$here/client.py" || status=$?
if [ "$status" -ne 0 ]; then
  echo "--- server log ---"
  cat "$work/server.log"
fi
exit "$status"
