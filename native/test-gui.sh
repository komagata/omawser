#!/bin/sh
set -eu
root=$(mktemp -d /tmp/omawser-qt-test-XXXXXX)
trap 'rm -rf "$root"' EXIT HUP INT TERM
export XDG_CONFIG_HOME="$root/config" XDG_CACHE_HOME="$root/cache"
export XDG_DATA_HOME="$root/data" XDG_STATE_HOME="$root/state"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME/omarchy/current/theme"
printf "background='#1a1b26'\nforeground='#c0caf5'\naccent='#7aa2f7'\nmode='dark'\n" > "$XDG_STATE_HOME/omarchy/current/theme/colors.toml"
"$(dirname "$0")/build/gui-test" "$@"
