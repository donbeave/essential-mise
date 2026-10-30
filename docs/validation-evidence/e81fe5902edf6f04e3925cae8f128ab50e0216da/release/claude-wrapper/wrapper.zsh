claude() {
  local statusline="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}/bin/claude-statusline.sh"
  local statusline_command settings
  printf -v statusline_command '%q' "$statusline"
  settings="$(/opt/homebrew/bin/jq -cn --arg command "$statusline_command" '{statusLine:{type:"command",command:$command,padding:0,refreshInterval:5}}')" || return
  BASH_COMPAT=3.2 command claude --settings "$settings" "$@"
}
