#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

[[ $# -eq 1 ]] || fail 'Usage: VM_NAME=essential-mise-test-NAME run-test-scenario.sh <stage0|standard|ai|ai-desktop|amp-conflict|claude-conflict|unapply|prune-review|java|rust|login-items|omp-config|mas-signed-out|full-dry-run|preferences>'
scenario="$1"
case "$VM_NAME" in
  essential-mise-test-*) ;;
  *) fail 'Application scenarios require a registered disposable VM' ;;
esac
case "$scenario" in
  stage0|standard|ai|ai-desktop|amp-conflict|claude-conflict|unapply|prune-review|java|rust|login-items|omp-config|mas-signed-out|full-dry-run|preferences) ;;
  *) fail "Unknown test scenario: $scenario" ;;
esac
vm_require_name
vm_require_running
vm_scan_host_key
evidence_dir="/tmp/essential-mise-evidence/$scenario"
vm_ssh "mkdir -p '$evidence_dir'"
reviewed_commit="${VM_REVIEWED_COMMIT:-}"
config_dir="/Users/$VM_SSH_USER/.config/mise"
printf 'scenario=%s\nreviewed_commit=%s\n' "$scenario" "$reviewed_commit" > "$evidence_dir/scenario.env"
vm_ssh "\$HOME/.local/bin/mise --version > '$evidence_dir/mise-version.txt'"
if vm_ssh "test -d '$config_dir/.git'"; then
  vm_ssh "git -C '$config_dir' rev-parse HEAD > '$evidence_dir/config-revision.txt'"
fi

run_logged() {
  local name="$1" command="$2"
  vm_ssh "set -o pipefail; { $command; } 2>&1 | tee '$evidence_dir/$name.log'"
}

stage0_command="set -e
reviewed_commit='$reviewed_commit'
essential_mise_script=\"\$(/usr/bin/curl -fsSL \"https://raw.githubusercontent.com/donbeave/essential-mise/\$reviewed_commit/bootstrap.sh\")\" && test -n \"\$essential_mise_script\" && /bin/bash -c \"\$essential_mise_script\""

case "$scenario" in
  stage0)
    [[ "$reviewed_commit" =~ ^[0-9a-f]{40}$ ]] || fail 'Stage 0 requires VM_REVIEWED_COMMIT with a full reviewed commit SHA'
    run_logged first-run "$stage0_command"
    vm_ssh "printf 'env = [\"ai-cli\", \"system-preferences\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged optional-selection-rerun "$stage0_command"
    vm_ssh "git -C '$config_dir' remote set-url origin 'https://example.invalid/essential-mise.git'"
    if run_logged wrong-origin "$stage0_command"; then
      outcome=unexpected-success
    else
      outcome=expected-refusal
    fi
    vm_ssh "git -C '$config_dir' remote set-url origin 'https://github.com/donbeave/essential-mise.git'"
    [[ "$outcome" == expected-refusal ]] || fail 'Stage 0 accepted an unexpected origin'
    run_logged shell-verification "/bin/zsh -lic 'cd /tmp && command -v git && command -v brew && command -v mise && mise --version && mise -E foundation doctor'"
    ;;
  standard)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E foundation,terminal,development bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E foundation,terminal,development bootstrap"
    run_logged second-run "cd '$config_dir' && \$HOME/.local/bin/mise -E foundation,terminal,development bootstrap"
    run_logged status "cd '$config_dir' && \$HOME/.local/bin/mise -E foundation,terminal,development bootstrap status --missing"
    vm_ssh "printf 'env = [\"foundation\", \"terminal\", \"development\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged shell-verification "cd /tmp && /bin/zsh -lic 'command -v starship && command -v bat && command -v fish && test -x /Applications/Ghostty.app/Contents/MacOS/ghostty && test \"\$(git -C \"\$HOME/.oh-my-zsh\" rev-parse HEAD)\" = 0b50afdbfd8cbd0e03be68ecec4e1b5d3f55ef0a && test \"\$(git -C \"\$HOME/.oh-my-zsh/custom/plugins/zsh-autosuggestions\" rev-parse HEAD)\" = 85919cd1ffa7d2d5412f6d3fe437ebdbeeec4fc5 && mise doctor' && /opt/homebrew/bin/fish -lc 'mise --version'"
    ;;
  ai)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-tools,ai-cli bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-tools,ai-cli bootstrap"
    run_logged status "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-tools,ai-cli bootstrap status --missing"
    run_logged second-run "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-tools,ai-cli bootstrap"
    run_logged cli-verification "/bin/zsh -lic 'cd /tmp && rg --version && fd --version && fff-mcp --healthcheck && rtk --version && agent-browser --version && amp --version && omp --version && command -v claude && command -v codex && command -v kimi && command -v opencode'"
    ;;
  ai-desktop)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-desktop bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-desktop bootstrap"
    run_logged second-run "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-desktop bootstrap"
    run_logged verification "/opt/homebrew/bin/brew list --cask amp@beta && test -d /Applications/Amp.app && mdls -name kMDItemVersion /Applications/Amp.app"
    ;;
  amp-conflict)
    vm_ssh 'mkdir -p /Applications/Amp.app/Contents && printf unmanaged > /Applications/Amp.app/Contents/essential-mise-test-marker'
    if run_logged existing-app-refusal "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-desktop bootstrap --only packages"; then
      outcome=unexpected-success
    else
      outcome=expected-refusal
    fi
    vm_ssh 'test "$(/bin/cat /Applications/Amp.app/Contents/essential-mise-test-marker)" = unmanaged && ! /opt/homebrew/bin/brew list --cask amp@beta >/dev/null 2>&1'
    [[ "$outcome" == expected-refusal ]] || fail 'Amp hook accepted an unmanaged existing app'
    ;;
  claude-conflict)
    run_logged install-existing "/opt/homebrew/bin/brew install --cask claude-code@latest"
    run_logged existing-preview "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-cli bootstrap --dry-run"
    run_logged existing-apply "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-cli bootstrap"
    run_logged second-run "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-cli bootstrap"
    run_logged cli-verification "/bin/zsh -lic 'claude --version'"
    ;;
  unapply)
    run_logged unapply "cd '$config_dir' && \$HOME/.local/bin/mise bootstrap unapply terminal"
    run_logged verify-removed 'test ! -e "$HOME/.config/ghostty/config" && test ! -e "$HOME/.config/starship.toml" && ! /usr/bin/grep -F "source \"\$ZSH/oh-my-zsh.sh\"" "$HOME/.zshrc" && test -d "$HOME/.oh-my-zsh"'
    run_logged reapply "cd '$config_dir' && \$HOME/.local/bin/mise -E terminal bootstrap"
    run_logged verify-restored 'test -f "$HOME/.config/ghostty/config" && test -f "$HOME/.config/starship.toml" && /usr/bin/grep -F "source \"\$ZSH/oh-my-zsh.sh\"" "$HOME/.zshrc"'
    ;;
  prune-review)
    run_logged install-unrelated "/opt/homebrew/bin/brew install sl"
    run_logged candidates "cd '$config_dir' && \$HOME/.local/bin/mise bootstrap packages prune --dry-run"
    vm_ssh '/usr/bin/grep -E "(^|[[:space:]])sl([[:space:]]|$)" /tmp/essential-mise-evidence/prune-review/candidates.log'
    run_logged manual-cleanup "/opt/homebrew/bin/brew uninstall sl"
    vm_ssh '! /opt/homebrew/bin/brew list --formula | /usr/bin/grep -Fx sl'
    ;;
  java)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E development-java bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E development-java bootstrap"
    run_logged exec-check "cd '$config_dir' && \$HOME/.local/bin/mise -E development-java exec -- java -version"
    run_logged build-tools "/bin/zsh -lic 'mvn -version && gradle -version'"
    vm_ssh "printf 'env = [\"development-java\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged persistent "/bin/zsh -lic 'cd /tmp && java -version && case \"\$(mise which java)\" in *\".local/share/mise/installs/\"*) ;; *) exit 1;; esac && /opt/homebrew/bin/brew list --versions openjdk || true'"
    ;;
  rust)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust bootstrap"
    run_logged exec-check "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust exec -- cargo --version"
    run_logged tool-check "/bin/zsh -lic 'cargo --version && rustc --version && mr-boxington --version'"
    vm_ssh "printf 'env = [\"development-rust\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged persistent "/bin/zsh -lic 'cd /tmp && cargo --version && case \"\$(mise which cargo)\" in *\".local/share/mise/installs/\"*) ;; *) exit 1;; esac'"
    ;;
  login-items)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E utilities bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E utilities bootstrap"
    run_logged status "cd '$config_dir' && \$HOME/.local/bin/mise bootstrap macos login-items status --missing"
    run_logged verify 'test -d "/Applications/CleanShot X.app" && test -d "/Applications/Bartender 7.app"'
    ;;
  omp-config)
    run_logged config "/opt/homebrew/bin/omp config"
    run_logged models "/opt/homebrew/bin/omp models"
    run_logged instructions 'test -f "$HOME/.omp/agent/AGENTS.md"'
    ;;
  mas-signed-out)
    run_logged cli "/opt/homebrew/bin/mas list"
    run_logged metadata "/opt/homebrew/bin/mas info 1037126344"
    if run_logged account "/opt/homebrew/bin/mas account"; then
      outcome=unexpected-success
    else
      outcome=expected-refusal
    fi
    [[ "$outcome" == expected-refusal ]] || fail 'MAS account unexpectedly succeeded without sign-in'
    ;;
  full-dry-run)
    run_logged all-environments "cd '$config_dir' && \$HOME/.local/bin/mise -C /tmp -E foundation,terminal,development,security,js,development-rust,development-java,development-apps,development-containers,development-editors,development-apple,kubernetes,infrastructure,utilities,messengers,browsers,privacy,productivity,office,design,media,network,admin,ai-tools,ai-cli,ai-desktop,system-preferences bootstrap --dry-run"
    ;;
  preferences)
    vm_ssh 'set -e
defaults write NSGlobalDomain EssentialMiseValidation -string preserve
defaults write NSGlobalDomain AppleFirstWeekday -dict gregorian -int 1 syntheticKeep -string keep
defaults write NSGlobalDomain AppleICUDateFormatStrings -dict 0 "y-MM-dd HH:mm" 1 "old" 2 "keep"
mkdir -p /tmp/essential-mise-evidence/preferences
defaults export NSGlobalDomain /tmp/essential-mise-evidence/preferences/global-before.plist
defaults export com.apple.dock /tmp/essential-mise-evidence/preferences/dock-before.plist
defaults export com.apple.finder /tmp/essential-mise-evidence/preferences/finder-before.plist'
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E system-preferences bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E system-preferences bootstrap"
    run_logged stored-status "cd '$config_dir' && \$HOME/.local/bin/mise -E system-preferences bootstrap macos defaults status --missing"
    run_logged behavior-verification 'set -e
test "$(defaults read NSGlobalDomain EssentialMiseValidation)" = preserve
defaults export NSGlobalDomain /tmp/essential-mise-evidence/preferences/global-after.plist
test "$(plutil -extract AppleFirstWeekday.gregorian raw -o - /tmp/essential-mise-evidence/preferences/global-after.plist)" = 2
test "$(plutil -extract AppleFirstWeekday.syntheticKeep raw -o - /tmp/essential-mise-evidence/preferences/global-after.plist)" = keep
test "$(plutil -extract AppleICUDateFormatStrings.0 raw -o - /tmp/essential-mise-evidence/preferences/global-after.plist)" = "y-MM-dd HH:mm"
test "$(plutil -extract AppleICUDateFormatStrings.1 raw -o - /tmp/essential-mise-evidence/preferences/global-after.plist)" = y-MM-dd
test "$(plutil -extract AppleICUDateFormatStrings.2 raw -o - /tmp/essential-mise-evidence/preferences/global-after.plist)" = keep
test "$(defaults read com.apple.dock autohide)" = 1
test "$(defaults read com.apple.dock tilesize)" = 58'
    ;;
esac
printf 'Guest scenario %s completed on %s. Review logs before assigning release status.\n' "$scenario" "$VM_NAME"
