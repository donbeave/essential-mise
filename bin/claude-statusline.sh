#!/usr/bin/env bash
# Claude Code status line: dir, model, effort, context %, plus one line per running subagent.
input=$(cat)

dim=$'\e[2m'; cyan=$'\e[36m'; mag=$'\e[35m'; yel=$'\e[33m'; grn=$'\e[32m'; red=$'\e[31m'; rst=$'\e[0m'

fmt_k() { awk -v n="$1" 'BEGIN { if (n >= 1000000) printf "%.1fM", n/1000000; else if (n >= 1000) printf "%dk", n/1000; else printf "%d", n }'; }

pct_color() {
  if (( $1 >= 80 )); then printf %s "$red"; elif (( $1 >= 50 )); then printf %s "$yel"; else printf %s "$grn"; fi
}

IFS=$'\t' read -r cwd model effort used_pct ctx_size ctx_used transcript < <(jq -r '[
  (.workspace.current_dir // .cwd // "?"),
  (.model.display_name // .model.id // "?"),
  (.effort.level // "-"),
  (.context_window.used_percentage // 0),
  (.context_window.context_window_size // 0),
  ((.context_window.current_usage // {}) | (.input_tokens // 0) + (.cache_read_input_tokens // 0) + (.cache_creation_input_tokens // 0)),
  (.transcript_path // "-")
] | @tsv' <<<"$input")

used_pct=${used_pct:-0}
c=$(pct_color "${used_pct%.*}")
printf '%s%s%s %s│%s %s%s%s %s %s│%s %s%s%%%s %s(%s/%s)%s' \
  "$cyan" "${cwd/#$HOME/~}" "$rst" "$dim" "$rst" "$mag" "$model" "$rst" "$effort" "$dim" "$rst" \
  "$c" "$used_pct" "$rst" "$dim" "$(fmt_k "$ctx_used")" "$(fmt_k "$ctx_size")" "$rst"

# Running subagents: transcripts live in <session>/subagents/agent-*.jsonl with a .meta.json beside each.
sub_dir="${transcript%.jsonl}/subagents"
[[ -d $sub_dir ]] || exit 0
shopt -s nullglob
for meta in "$sub_dir"/agent-*.meta.json; do
  [[ $(jq -r '.finished // false' "$meta") == true ]] && continue
  log="${meta%.meta.json}.jsonl"
  [[ -f $log ]] || continue
  # A killed or crashed agent is never marked finished; hide it once its transcript is idle for 10 minutes.
  (( $(date +%s) - $(stat -f %m "$log") > 600 )) && continue
  # An interrupted agent is not marked finished either; its last entry is the interrupt marker.
  tail -n 1 "$log" | grep -q '\[Request interrupted by user' && continue
  IFS=$'\t' read -r atype desc aeffort < <(jq -r '[.agentType // "agent", .description // "", .effort // ""] | @tsv' "$meta")
  # The meta file does not record effort; fall back to the agent definition, then to the session's effort.
  if [[ -z $aeffort ]]; then
    def="$HOME/.claude/agents/$atype.md"
    [[ -f "$CLAUDE_PROJECT_DIR/.claude/agents/$atype.md" ]] && def="$CLAUDE_PROJECT_DIR/.claude/agents/$atype.md"
    [[ -f $def ]] && aeffort=$(sed -n 's/^effort:[[:space:]]*//p' "$def" | head -n 1)
    aeffort=${aeffort:-$effort}
  fi
  IFS=$'\t' read -r amodel atokens < <(tail -n 200 "$log" | grep '"type":"assistant"' | tail -n 1 |
    jq -r '[(.message.model // "?" | sub("^claude-"; "")), (.message.usage // {} | (.input_tokens // 0) + (.cache_read_input_tokens // 0) + (.cache_creation_input_tokens // 0))] | @tsv')
  amodel=${amodel%%-*}
  printf '\n%s↳%s %s%s%s %s│%s %s %s %s│%s %s' \
    "$dim" "$rst" "$yel" "${atype#ca-}" "$rst" "$dim" "$rst" "${amodel:-starting}" "$aeffort" "$dim" "$rst" \
    "$(fmt_k "${atokens:-0}")"
done
