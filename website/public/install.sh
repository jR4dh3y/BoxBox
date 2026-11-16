#!/usr/bin/env bash
# =============================================================================
# BoxBox setup script
# =============================================================================
# BoxBox is a file manager you open in your web browser. It runs on your own
# computer or server and shows the folders you choose.
#
# This script does five things, in order:
#   1. Checks that the programs it needs are installed.
#   2. Downloads two small text files that tell Docker how to run BoxBox.
#   3. Asks you to choose a password.
#   4. Saves your settings in a file called ".env".
#   5. Starts BoxBox.
#
# It does not need root, does not install anything on its own, and only
# creates files inside one folder (called "boxbox" unless you say otherwise).
#
# Run it with no options to accept the defaults, or add options like this:
#
#   bash install.sh --folder /srv/media --port 9000 --user sam
#
# Every option is explained in the "Your settings" section below.
# =============================================================================

# Stop at the first thing that goes wrong, instead of carrying on half-done.
set -euo pipefail

# -----------------------------------------------------------------------------
# Your settings
# -----------------------------------------------------------------------------

# --folder  The folder on this computer that BoxBox will show as "home".
#           Everything inside it can be browsed, uploaded to, and shared.
#           If you leave it out, your own home folder is used.
FOLDER="${HOME}"

# --port    The number at the end of the address you will open in a browser.
#           With 8080 the address is http://localhost:8080. Change it only
#           if another program on this computer already uses that number.
PORT="8080"

# --user    The name you will type to sign in to BoxBox. It is separate from
#           your computer login. Letters, digits and "_" only.
USER_NAME="admin"

# --dir     Where this script keeps BoxBox's own settings files. It is not
#           where your files live. A folder with this name is created in
#           the place you run the script from.
DIR="boxbox"

# Where the two Docker files are downloaded from: the BoxBox project itself.
SOURCE="https://raw.githubusercontent.com/jR4dh3y/BoxBox/master"

# -----------------------------------------------------------------------------
# Helpers: small messages so you can see what is happening
# -----------------------------------------------------------------------------

say()  { printf '\n==> %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }
stop() { printf '\nStopped: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'HELP'
BoxBox setup

  bash install.sh [--folder PATH] [--port NUMBER] [--user NAME] [--dir NAME]

  --folder PATH   Folder to show in BoxBox        (default: your home folder)
  --port NUMBER   Port for the web address        (default: 8080)
  --user NAME     Name you sign in with           (default: admin)
  --dir NAME      Folder for BoxBox's own files   (default: boxbox)
  --help          Show this message
HELP
}

# -----------------------------------------------------------------------------
# Read the options you gave on the command line
# -----------------------------------------------------------------------------

while [ "$#" -gt 0 ]; do
  case "$1" in
    --folder) FOLDER="${2:-}"; shift 2 || stop "--folder needs a path after it." ;;
    --port)   PORT="${2:-}"; shift 2 || stop "--port needs a number after it." ;;
    --user)   USER_NAME="${2:-}"; shift 2 || stop "--user needs a name after it." ;;
    --dir)    DIR="${2:-}"; shift 2 || stop "--dir needs a name after it." ;;
    --help|-h) usage; exit 0 ;;
    *) usage; stop "I do not know the option \"$1\"." ;;
  esac
done

# Check the settings before touching anything, so a typo cannot cause harm.
case "$FOLDER" in
  /*) ;;
  *) stop "--folder must be a full path that starts with \"/\", like /home/sam." ;;
esac
[[ "$FOLDER" =~ ^[A-Za-z0-9._/-]+$ ]] || stop "--folder may only contain letters, digits, \".\", \"_\", \"-\" and \"/\"."
[[ "$PORT" =~ ^[0-9]{1,5}$ ]] && [ "$PORT" -ge 1 ] && [ "$PORT" -le 65535 ] || stop "--port must be a number from 1 to 65535."
[[ "$USER_NAME" =~ ^[A-Za-z][A-Za-z0-9_]{0,31}$ ]] || stop "--user must start with a letter and use only letters, digits and \"_\"."
[ -n "$DIR" ] || stop "--dir cannot be empty."

# -----------------------------------------------------------------------------
# Step 1 of 5: check that the needed programs are installed
# -----------------------------------------------------------------------------
# Docker runs BoxBox in its own sealed box, so nothing else on your computer
# has to change. curl downloads files. openssl makes a random secret.

say "Step 1 of 5: checking what is installed"

command -v curl >/dev/null    || stop "\"curl\" is missing. Install it with your system's package manager, then run this again."
command -v openssl >/dev/null || stop "\"openssl\" is missing. Install it with your system's package manager, then run this again."
command -v docker >/dev/null  || stop "Docker is missing. Install it from https://docs.docker.com/engine/install/ and run this again."
docker compose version >/dev/null 2>&1 || stop "Docker is installed, but its \"compose\" add-on is missing. See https://docs.docker.com/compose/install/"
docker info >/dev/null 2>&1   || stop "Docker is installed but not usable right now. Start it, or check that your user is allowed to use it (see https://docs.docker.com/engine/install/linux-postinstall/)."

[ -d "$FOLDER" ] || stop "The folder $FOLDER does not exist. Pick a folder that is already there with --folder."
note "Everything needed is here."

# BoxBox runs as its own unprivileged user (number 10001), not as you. If your
# folder is closed to other users, BoxBox will open but show it as empty.
if [ -z "$(find "$FOLDER" -maxdepth 0 -perm -o+rx 2>/dev/null)" ]; then
  note "Heads up: $FOLDER is not readable by other users on this computer."
  note "BoxBox may show it as empty. The fix is explained here:"
  note "https://boxbox.radhey.dev/docs/docker/#permission-notes"
fi

# -----------------------------------------------------------------------------
# Step 2 of 5: download the two Docker files
# -----------------------------------------------------------------------------
# docker-compose.yml says which BoxBox version to run and which folders to
# hand it. config.yaml lists the folders BoxBox shows in its sidebar.

say "Step 2 of 5: downloading BoxBox's Docker files into \"$DIR\""

# Never overwrite an existing setup: its .env holds your login and secret.
[ ! -e "$DIR/.env" ] || stop "BoxBox is already set up in \"$DIR\". To change a setting, edit \"$DIR/.env\". To start over, delete that folder first."

mkdir -p "$DIR/backend"
cd "$DIR"
curl -fsSL "$SOURCE/docker-compose.yml" -o docker-compose.yml || stop "Could not download docker-compose.yml. Check your internet connection."
curl -fsSL "$SOURCE/backend/config.yaml" -o backend/config.yaml || stop "Could not download config.yaml. Check your internet connection."
note "Downloaded docker-compose.yml and backend/config.yaml."

# -----------------------------------------------------------------------------
# Step 3 of 5: choose a password
# -----------------------------------------------------------------------------
# You type it twice so a typo cannot lock you out. Nothing appears on screen
# while you type; that is normal. The password itself is never saved. Only a
# scrambled form of it (a "hash") is, which cannot be turned back into it.
#
# If the BOXBOX_PASSWORD variable is already set, it is used and no question
# is asked. That is for automated setups.

say "Step 3 of 5: choose the password for \"$USER_NAME\""

PASSWORD="${BOXBOX_PASSWORD:-}"
if [ -z "$PASSWORD" ]; then
  # /dev/tty is your keyboard. It is read directly because, when this script
  # is piped in from curl, the script itself is arriving on the usual input.
  { : </dev/tty; } 2>/dev/null || stop "No keyboard to ask for a password. Set BOXBOX_PASSWORD and run this again."
  read -rsp "    Password: " PASSWORD </dev/tty; printf '\n'
  read -rsp "    Same password again: " AGAIN </dev/tty; printf '\n'
  [ -n "$PASSWORD" ] || stop "The password cannot be empty."
  [ "$PASSWORD" = "$AGAIN" ] || stop "The two passwords were not the same. Nothing was saved; run this again."
fi

# htpasswd is the standard tool that scrambles passwords. If it is not
# installed, the same tool is borrowed from a small official Docker image.
# Either way the password is passed privately, not on a command line where
# other programs could see it.
if command -v htpasswd >/dev/null; then
  HASH="$(printf '%s\n' "$PASSWORD" | htpasswd -niBC 12 "$USER_NAME")"
else
  note "Borrowing the password tool from Docker (a small one-time download)."
  HASH="$(printf '%s\n' "$PASSWORD" | docker run --rm -i httpd:2.4-alpine htpasswd -niBC 12 "$USER_NAME")"
fi
# The tool prints "name:hash"; keep only the hash.
HASH="$(printf '%s' "$HASH" | head -n 1 | cut -d: -f2)"
case "$HASH" in
  '$2'*) ;;
  *) stop "Could not scramble the password. Nothing was saved; run this again." ;;
esac
unset PASSWORD AGAIN

# -----------------------------------------------------------------------------
# Step 4 of 5: save your settings
# -----------------------------------------------------------------------------
# ".env" is a plain text file of NAME=value lines that Docker reads when it
# starts BoxBox. You can open and edit it later with any text editor.

say "Step 4 of 5: saving your settings to \"$DIR/.env\""

# Only you may read the file, because it holds the secret below.
umask 077
cat > .env <<EOF
# BoxBox settings, written by install.sh. After changing a line, run
# "docker compose up -d" in this folder to apply it.

# A long random secret BoxBox uses to recognise signed-in browsers.
# Never share it. Changing it signs everyone out.
BOXBOX_JWT_SECRET=$(openssl rand -base64 32)

# Your login. The part after BOXBOX_USERS_ is the user name; the value is
# the scrambled password. Keep the single quotes.
BOXBOX_USERS_${USER_NAME}='${HASH}'

# The port: BoxBox answers at http://localhost:<this number>.
HOST_PORT=${PORT}

# The folder on this computer that BoxBox shows as "home".
HOME_PATH=${FOLDER}
EOF
note "Saved."

# -----------------------------------------------------------------------------
# Step 5 of 5: start BoxBox
# -----------------------------------------------------------------------------
# "up -d" downloads BoxBox the first time, then starts it in the background.
# It keeps running after you close this window and after a reboot.

say "Step 5 of 5: starting BoxBox (the first start downloads it)"
docker compose up -d

say "BoxBox is running."
note "Open:     http://localhost:${PORT}"
note "Sign in:  ${USER_NAME}, with the password you just chose"
note ""
note "Stop it:    cd \"$PWD\" && docker compose down"
note "Update it:  cd \"$PWD\" && docker compose pull && docker compose up -d"
