#!/usr/bin/env bash
# Runs as root inside the user and mount namespace that run.sh creates.
set -euo pipefail
work=$1
client=$2
root=$work/root
mkdir -p "$root/tank/alice" "$root/tank/bob" "$root/tank/shared" "$root/data"
chmod 0755 "$root" "$root/tank"
chown 3000:3000 "$root/tank/alice" && chmod 0700 "$root/tank/alice"
chown 3001:3001 "$root/tank/bob" && chmod 0700 "$root/tank/bob"
chown 3000:4000 "$root/tank/shared" && chmod 2770 "$root/tank/shared"
chown 10001:10001 "$root/data"
# Upload chunks are staged in /tmp/boxbox; give this run a private one.
mkdir -p /tmp/boxbox
mount -t tmpfs tmpfs /tmp/boxbox
chown 10001:10001 /tmp/boxbox && chmod 0700 /tmp/boxbox

hash() { python3 -c 'import bcrypt, sys; print(bcrypt.hashpw(sys.argv[1].encode(), bcrypt.gensalt(10)).decode())' "$1"; }
cat >"$root/config.yaml" <<EOF
host: 127.0.0.1
port: 18080
jwt_secret: "nas-e2e-secret-that-is-long-enough-for-validation"
data_dir: $root/data
users:
  alice: "$(hash alice-pass)"
  bob: "$(hash bob-pass)"
  carol: "$(hash carol-pass)"
mount_points:
  - name: tank
    path: $root/tank
nas:
  identities:
    alice: { uid: 3000, gid: 3000, groups: [4000] }
    bob: { uid: 3001, gid: 3001, groups: [4000] }
EOF
chmod 0644 "$root/config.yaml"

# Like Compose with user 0:0, cap_drop ALL, cap_add SETUID and SETGID, and
# no-new-privileges.
setpriv --bounding-set=-all,+setuid,+setgid --inh-caps=-all --no-new-privs \
  "$work/server" --config "$root/config.yaml" >"$work/server.log" 2>&1 &
supervisor=$!
for _ in $(seq 50); do
  curl -sf http://127.0.0.1:18080/health >/dev/null && break
  sleep 0.1
done

status=0
python3 -I "$client" "$root" "$supervisor" || status=$?

kill -TERM "$supervisor"
for _ in $(seq 100); do
  kill -0 "$supervisor" 2>/dev/null || break
  sleep 0.1
done
sleep 0.5
left=$(pgrep -f "^$work/server" || true)
if [ -n "$left" ]; then
  echo "FAIL processes left after shutdown: $left"
  status=1
else
  echo "PASS supervisor, API and workers all exited on SIGTERM"
fi
if grep -q "Server shutdown complete" "$work/server.log"; then
  echo "PASS API shut down gracefully"
else
  echo "FAIL API did not shut down gracefully"
  status=1
fi
rm -rf "$root"
exit "$status"
