#!/bin/bash
set -u
script_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)" || exit 1
status=0
for plugin_script in pstack-plugins.sh adhd-plugins.sh; do
  if /bin/bash "$script_dir/$plugin_script"; then
    :
  else
    printf '%s failed\n' "$plugin_script" >&2
    status=1
  fi
done
exit "$status"
