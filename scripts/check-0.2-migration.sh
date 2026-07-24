#!/usr/bin/env bash
set -u

env_file="${BOXBOX_MIGRATION_ENV_FILE:-.env}"
compose_file="${BOXBOX_MIGRATION_COMPOSE_FILE:-docker-compose.yml}"
found_legacy=0

mark_legacy() {
	found_legacy=1
}

print_env_replacements() {
	cat <<'EOF'

Rename deprecated environment variables:
  FM_JWT_SECRET       -> BOXBOX_JWT_SECRET
  FM_USERS_<username> -> BOXBOX_USERS_<username>
  FM_RATE_LIMIT_RPS   -> BOXBOX_RATE_LIMIT_RPS
  FM_ALLOWED_ORIGINS  -> BOXBOX_ALLOWED_ORIGINS
  FM_PORT             -> BOXBOX_PORT
  FM_HOST             -> BOXBOX_HOST
  FM_MAX_UPLOAD_MB    -> BOXBOX_MAX_UPLOAD_MB
  FM_CHUNK_SIZE_MB    -> BOXBOX_CHUNK_SIZE_MB

BOXBOX_* values take precedence when both old and new variables are set.
EOF
}

check_env_file() {
	if [[ ! -f "$env_file" ]]; then
		return
	fi

	if grep -Eq '^[[:space:]]*(export[[:space:]]+)?FM_[A-Za-z0-9_]*=' "$env_file"; then
		mark_legacy
		echo "Deprecated FM_* variables found in $env_file:"
		grep -En '^[[:space:]]*(export[[:space:]]+)?FM_[A-Za-z0-9_]*=' "$env_file" | sed 's/^/  /'
		print_env_replacements
	fi
}

check_compose_file() {
	if [[ ! -f "$compose_file" ]]; then
		return
	fi

	if grep -Eq '(^[[:space:]]*filemanager:[[:space:]]*$|container_name:[[:space:]]*filemanager[[:space:]]*$|filemanager-(data|temp))' "$compose_file"; then
		mark_legacy
		echo
		echo "Deprecated Compose names found in $compose_file:"
		grep -En '(^[[:space:]]*filemanager:[[:space:]]*$|container_name:[[:space:]]*filemanager[[:space:]]*$|filemanager-(data|temp))' "$compose_file" | sed 's/^/  /'
		cat <<'EOF'

Recommended Compose renames:
  service name:    filemanager      -> boxbox
  container_name:  filemanager      -> boxbox
  named volume:    filemanager-data -> boxbox-data
  named volume:    filemanager-temp -> boxbox-temp
EOF
	fi
}

print_volume_copy_command() {
	local legacy_volume="$1"
	local replacement_volume="$2"

	cat <<EOF

docker volume create $replacement_volume
docker run --rm \\
  -v $legacy_volume:/from:ro \\
  -v $replacement_volume:/to \\
  alpine sh -c 'cd /from && cp -a . /to'
EOF
}

replacement_volume_name() {
	local legacy_volume="$1"

	case "$legacy_volume" in
		filemanager-data)
			printf '%s\n' "boxbox-data"
			;;
		filemanager-temp)
			printf '%s\n' "boxbox-temp"
			;;
		*_filemanager-data)
			printf '%s\n' "${legacy_volume%_filemanager-data}_boxbox-data"
			;;
		*_filemanager-temp)
			printf '%s\n' "${legacy_volume%_filemanager-temp}_boxbox-temp"
			;;
	esac
}

find_legacy_volumes() {
	docker volume ls --format '{{.Name}}' | grep -E '(^|_)filemanager-(data|temp)$' || true
}

check_docker_volumes() {
	if ! command -v docker >/dev/null 2>&1; then
		echo
		echo "Docker was not found; skipped Docker volume inspection."
		return
	fi

	if ! docker info >/dev/null 2>&1; then
		echo
		echo "Docker is installed but the daemon is unavailable; skipped Docker volume inspection."
		return
	fi

	local legacy_volumes
	legacy_volumes="$(find_legacy_volumes)"

	if [[ -z "$legacy_volumes" ]]; then
		return
	fi

	mark_legacy
	echo
	echo "Deprecated Docker volumes found:"
	printf '%s\n' "$legacy_volumes" | sed 's/^/  /'

	cat <<'EOF'

Safe manual volume copy flow:
  1. Stop BoxBox before copying volumes.
  2. Copy only from old filemanager-* volumes into new boxbox-* volumes.
  3. Start the new Compose config only after every needed copy command succeeds.
  4. Do not delete old filemanager-* volumes during the migration.

docker compose down
EOF

	while IFS= read -r legacy_volume; do
		[[ -n "$legacy_volume" ]] || continue
		print_volume_copy_command "$legacy_volume" "$(replacement_volume_name "$legacy_volume")"
	done <<< "$legacy_volumes"

	cat <<'EOF'

docker compose up -d

If a copy command fails, the old filemanager-* volume is still the source of truth.
The new boxbox-* target volume may be partial; remove/recreate the partial target
or copy into a fresh target before starting the new Compose config.
EOF
}

cat <<'EOF'
BoxBox v0.2 migration check
This script is check-only. It does not edit files, stop containers, or change Docker volumes.
EOF

check_env_file
check_compose_file
check_docker_volumes

if [[ "$found_legacy" -eq 1 ]]; then
	cat <<'EOF'

Legacy deployment settings were detected. BoxBox v0.2.x accepts FM_* aliases,
but rename them before a future breaking release. See docs/release.md for the full upgrade notes.
EOF
	exit 2
fi

echo
echo "No BoxBox v0.2 legacy deployment settings detected."
exit 0
