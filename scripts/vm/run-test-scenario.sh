#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

[[ $# -eq 1 ]] || fail 'Usage: VM_NAME=essential-mise-test-NAME run-test-scenario.sh <stage0|standard|ai|ai-desktop|amp-conflict|surge-conflict|claude-conflict|unapply|prune-review|js|java|rust|login-items|omp-config|mas-signed-out|optional-modules|full-dry-run|preferences>'
scenario="$1"
case "$VM_NAME" in
  essential-mise-test-*) ;;
  *) fail 'Application scenarios require a registered disposable VM' ;;
esac
case "$scenario" in
  stage0|standard|ai|ai-desktop|amp-conflict|surge-conflict|claude-conflict|unapply|prune-review|js|java|rust|login-items|omp-config|mas-signed-out|optional-modules|full-dry-run|preferences) ;;
  *) fail "Unknown test scenario: $scenario" ;;
esac
vm_require_name
vm_require_running
vm_scan_host_key
evidence_dir="/tmp/essential-mise-evidence/$scenario"
vm_ssh "rm -rf '$evidence_dir' && mkdir -p '$evidence_dir'"
reviewed_commit="${VM_REVIEWED_COMMIT:-$(git rev-parse HEAD)}"
[[ "$reviewed_commit" =~ ^[0-9a-f]{40}$ ]] || fail 'A full reviewed commit SHA is required'
config_dir="/Users/$VM_SSH_USER/.config/mise"
fixture_sha256="$(shasum -a 256 "$0" | cut -d' ' -f1)"
vm_ssh "printf 'scenario=%s\nreviewed_commit=%s\nvm_name=%s\nfixture_sha256=%s\ntart_version=%s\n' '$scenario' '$reviewed_commit' '$VM_NAME' '$fixture_sha256' '$(tart --version)' > '$evidence_dir/scenario.env'"
vm_ssh "printf 'product_version=%s\nbuild_version=%s\n' \"\$(sw_vers -productVersion)\" \"\$(sw_vers -buildVersion)\" > '$evidence_dir/guest-os.txt'"
vm_scp "$0" "$VM_SSH_USER@$VM_IP:$evidence_dir/run-test-scenario.sh"
if vm_ssh 'test -x "$HOME/.local/bin/mise"'; then
  vm_ssh "\$HOME/.local/bin/mise --version > '$evidence_dir/mise-version.txt'"
fi
if vm_ssh "test -d '$config_dir/.git'"; then
  actual_commit="$(vm_ssh "git -C '$config_dir' rev-parse HEAD")"
  [[ "$actual_commit" == "$reviewed_commit" ]] || fail "Guest config $actual_commit does not match reviewed commit $reviewed_commit"
  vm_ssh "git -C '$config_dir' status --porcelain > '$evidence_dir/config-status.txt'"
  vm_ssh "test -z \"\$(git -C '$config_dir' status --porcelain)\"" || fail 'Guest config has uncommitted changes'
  vm_ssh "git -C '$config_dir' rev-parse HEAD > '$evidence_dir/config-revision.txt'"
fi

run_logged() {
  local name="$1" command="$2" result
  set +e
  vm_ssh "set -o pipefail; { $command; } 2>&1 | tee '$evidence_dir/$name.log'"
  result=$?
  set -e
  vm_ssh "printf '%s\n' '$result' > '$evidence_dir/$name.exit'"
  return "$result"
}

run_logged_interactive() {
  local name="$1" command="$2" result
  local host_evidence="/tmp/essential-mise-host-evidence/$scenario"
  mkdir -p "$host_evidence"
  set +e
  VM_OWNER_PASSWORD_FILE="${VM_OWNER_PASSWORD_FILE:-/tmp/essential-mise-vm-owner-password}" \
  VM_AUTO_CONFIRM="${VM_AUTO_CONFIRM:-0}" \
    "$(dirname "$0")/owner-auth-ssh.sh" "$command" 2>&1 | tee "$host_evidence/$name.log"
  result=$?
  set -e
  vm_scp "$host_evidence/$name.log" "$VM_SSH_USER@$VM_IP:$evidence_dir/$name.log"
  vm_ssh "printf '%s\n' '$result' > '$evidence_dir/$name.exit'"
  return "$result"
}

record_adopted_state() {
  local adopted_commit
  adopted_commit="$(vm_ssh "git -C '$config_dir' rev-parse HEAD")"
  [[ "$adopted_commit" == "$reviewed_commit" ]] || \
    fail "Launcher $reviewed_commit adopted unexpected config $adopted_commit"
  vm_ssh "git -C '$config_dir' rev-parse HEAD > '$evidence_dir/adopted-config-revision.txt'"
  vm_ssh "git -C '$config_dir' status --porcelain > '$evidence_dir/adopted-config-status.txt'"
  vm_ssh "test -z \"\$(git -C '$config_dir' status --porcelain)\"" || fail 'Adopted config has uncommitted changes'
  vm_ssh "test -x \"\$HOME/.local/bin/mise\" && \$HOME/.local/bin/mise --version > '$evidence_dir/adopted-mise-version.txt'"
}

stage0_command="set -e
reviewed_commit='$reviewed_commit'
essential_mise_script=\"\$(/usr/bin/curl -fsSL \"https://raw.githubusercontent.com/donbeave/essential-mise/\$reviewed_commit/bootstrap.sh\")\" && test -n \"\$essential_mise_script\" && /bin/bash -c \"\$essential_mise_script\""

case "$scenario" in
  stage0)
    [[ "$reviewed_commit" =~ ^[0-9a-f]{40}$ ]] || fail 'Stage 0 requires VM_REVIEWED_COMMIT with a full reviewed commit SHA'
    run_logged cold-preconditions "test ! -e '$config_dir' && test ! -x /opt/homebrew/bin/brew && { /usr/bin/xcode-select -p >/dev/null 2>&1 && printf 'command_line_tools=present\n' || printf 'command_line_tools=absent\n'; } && printf 'homebrew=absent\nconfig=absent\n'"
    VM_AUTO_CONFIRM=1 run_logged_interactive cold-first-run "$stage0_command"
    record_adopted_state
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
    run_logged gh-verification "/bin/zsh -lic 'test \"\$(gh config get git_protocol)\" = https && test \"\$(gh config get accessible_prompter)\" = disabled && test \"\$(gh config get prefer_editor_prompt)\" = disabled && test \"\$(gh config get color_labels)\" = disabled && test \"\$(gh config get version)\" = 1 && gh alias list | /usr/bin/grep -E \"co:[[:space:]]+pr checkout\"'"
    run_logged git-automation-absent 'test ! -e "$HOME/.gitconfig/preferences" && test ! -e "$HOME/.config/git/ignore"'
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
    run_logged cold-preconditions 'test ! -e /Applications/Amp.app && ! /opt/homebrew/bin/brew list --cask amp@beta >/dev/null 2>&1'
    vm_ssh 'mkdir -p /Applications/Amp.app/Contents && printf unmanaged > /Applications/Amp.app/Contents/essential-mise-test-marker'
    if run_logged existing-app-refusal "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-desktop bootstrap --only packages"; then
      outcome=unexpected-success
    else
      outcome=expected-refusal
    fi
    vm_ssh 'test "$(/bin/cat /Applications/Amp.app/Contents/essential-mise-test-marker)" = unmanaged && ! /opt/homebrew/bin/brew list --cask amp@beta >/dev/null 2>&1'
    vm_ssh '/usr/bin/python3 -c '\''from pathlib import Path; assert "Existing /Applications/Amp.app requires manual ownership migration before using Homebrew." in Path("/tmp/essential-mise-evidence/amp-conflict/existing-app-refusal.log").read_text()'\'''
    [[ "$outcome" == expected-refusal ]] || fail 'Amp hook accepted an unmanaged existing app'
    ;;
  surge-conflict)
    run_logged verify-owner '/opt/homebrew/bin/brew list --cask surge && test -d /Applications/Surge.app'
    run_logged_interactive uninstall-existing '/opt/homebrew/bin/brew uninstall --cask surge'
    vm_ssh 'mkdir -p /Applications/Surge.app/Contents && printf unmanaged > /Applications/Surge.app/Contents/essential-mise-test-marker'
    if run_logged existing-app-refusal "cd '$config_dir' && \$HOME/.local/bin/mise -E network bootstrap --only packages --yes"; then
      outcome=unexpected-success
    else
      outcome=expected-refusal
    fi
    vm_ssh 'test "$(/bin/cat /Applications/Surge.app/Contents/essential-mise-test-marker)" = unmanaged && ! /opt/homebrew/bin/brew list --cask surge >/dev/null 2>&1'
    vm_ssh '/usr/bin/python3 -c '\''from pathlib import Path; assert "Existing Surge applications require manual ownership migration before using Homebrew." in Path("/tmp/essential-mise-evidence/surge-conflict/existing-app-refusal.log").read_text()'\'''
    [[ "$outcome" == expected-refusal ]] || fail 'Surge hook accepted an unmanaged existing app'
    vm_ssh "mv /Applications/Surge.app '$evidence_dir/Surge-unmanaged.app'"
    run_logged_interactive restore "cd '$config_dir' && \$HOME/.local/bin/mise -E network bootstrap --only packages --yes"
    run_logged verify-restored '/opt/homebrew/bin/brew list --cask surge && test -x /opt/homebrew/bin/surge-cli && test -x /opt/homebrew/sbin/surge-dhcpd && /usr/bin/codesign --verify --deep --strict /Applications/Surge.app'
    ;;
  claude-conflict)
    run_logged install-existing "/opt/homebrew/bin/brew install --cask claude-code@latest"
    run_logged existing-preview "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-cli bootstrap --dry-run"
    run_logged existing-apply "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-cli bootstrap"
    run_logged second-run "cd '$config_dir' && \$HOME/.local/bin/mise -E ai-cli bootstrap"
    run_logged cli-verification "/bin/zsh -lic 'claude --version'"
    ;;
  unapply)
    run_logged unapply "cd '$config_dir' && \$HOME/.local/bin/mise bootstrap unapply terminal --yes"
    run_logged verify-removed 'test ! -e "$HOME/.config/ghostty/config" && test ! -e "$HOME/.config/starship.toml" && ! /usr/bin/grep -F "source \"\$ZSH/oh-my-zsh.sh\"" "$HOME/.zshrc" && test -d "$HOME/.oh-my-zsh"'
    run_logged reapply "cd '$config_dir' && \$HOME/.local/bin/mise -E terminal bootstrap"
    run_logged verify-restored 'test -f "$HOME/.config/ghostty/config" && test -f "$HOME/.config/starship.toml" && /usr/bin/grep -F "source \"\$ZSH/oh-my-zsh.sh\"" "$HOME/.zshrc"'
    ;;
  prune-review)
    run_logged install-unrelated "/opt/homebrew/bin/brew install sl"
    run_logged candidates "cd '$config_dir' && \$HOME/.local/bin/mise bootstrap packages prune --dry-run"
    vm_ssh '/usr/bin/grep -F "brew:sl@" /tmp/essential-mise-evidence/prune-review/candidates.log'
    run_logged manual-cleanup "/opt/homebrew/bin/brew uninstall sl"
    vm_ssh '! /opt/homebrew/bin/brew list --formula | /usr/bin/grep -Fx sl'
    ;;
  js)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E js bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E js bootstrap"
    run_logged exec-check "cd '$config_dir' && \$HOME/.local/bin/mise -E js exec -- /bin/bash -c 'node --version && bun --version'"
    vm_ssh "printf 'env = [\"js\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged persistent "/bin/zsh -lic 'cd /tmp && node --version && bun --version && case \"\$(mise which node)\" in *\".local/share/mise/installs/node/26.9.0/\"*) ;; *) exit 1;; esac && case \"\$(mise which bun)\" in *\".local/share/mise/installs/bun/1.4.4/\"*) ;; *) exit 1;; esac'"
    ;;
  java)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E development-java bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E development-java bootstrap"
    run_logged exec-check "cd '$config_dir' && \$HOME/.local/bin/mise -E development-java exec -- java -version"
    run_logged build-tools "/bin/zsh -lic 'mvn -version && gradle -version'"
    vm_ssh "printf 'env = [\"development-java\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged persistent "/bin/zsh -lic 'cd /tmp && java -version && case \"\$(mise which java)\" in *\".local/share/mise/installs/\"*) ;; *) exit 1;; esac && { /opt/homebrew/bin/brew list --versions openjdk || true; }'"
    run_logged persistent-build-tools "/bin/zsh -lic 'cd /tmp && gradle --stop && mvn -version && gradle -version && mvn -version | /usr/bin/grep -F \"vendor: Oracle Corporation\" && mvn -version | /usr/bin/grep -F \".local/share/mise/installs/java/oracle-graalvm-\" && gradle -version | /usr/bin/grep -F \"(Oracle Corporation 25.0.4.1.1\" && gradle -version | /usr/bin/grep -F \".local/share/mise/installs/java/oracle-graalvm-\"'"
    ;;
  rust)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust bootstrap"
    run_logged exec-check "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust exec -- cargo --version"
    run_logged build-check "cd '$config_dir' && \$HOME/.local/bin/mise -E development-rust exec -- /bin/bash -c 'set -e; rm -rf /tmp/essential-mise-cargo-smoke; cargo new --quiet /tmp/essential-mise-cargo-smoke; cd /tmp/essential-mise-cargo-smoke; cargo check'"
    vm_ssh "printf 'env = [\"development-rust\"]\n' > '$config_dir/miserc.local.toml'"
    run_logged tool-check "/bin/zsh -lic 'cargo --version && rustc --version && mbx --version'"
    run_logged persistent "/bin/zsh -lic 'cd /tmp && cargo --version && test \"\$(command -v cargo)\" = \"\$HOME/.local/share/mise/command-wrappers/bin/cargo\" && test \"\$(mise which cargo)\" = \"\$HOME/.cargo/bin/cargo\" && rustup show active-toolchain'"
    ;;
  login-items)
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E utilities bootstrap --dry-run"
    run_logged apply "cd '$config_dir' && \$HOME/.local/bin/mise -E utilities bootstrap"
    run_logged status "cd '$config_dir' && /bin/zsh -lic 'mise -E utilities bootstrap status'"
    run_logged verify 'test -d "/Applications/CleanShot X.app" && test -d "/Applications/Bartender 7.app" && /bin/zsh -lic '\''mise -E utilities bootstrap status --json'\'' | /usr/bin/python3 -c '\''import json, sys; entries = json.load(sys.stdin)["macos_login_items"]["entries"]; assert {entry["path"]: entry["state"] for entry in entries} == {"/Applications/CleanShot X.app": "set", "/Applications/Bartender 7.app": "set"}'\'''
    ;;
  omp-config)
    run_logged config "/opt/homebrew/bin/omp config"
    run_logged config-scope "/opt/homebrew/bin/omp config --scope global --json > '$evidence_dir/config-scope.json'"
    run_logged model-roles "/opt/homebrew/bin/yq -oj '.modelRoles' '$config_dir/dotfiles/ai-cli/omp/config.yml' > '$evidence_dir/expected-model-roles.json' && /usr/bin/python3 -c 'import json; actual = json.load(open(\"$evidence_dir/config-scope.json\"))[\"modelRoles\"][\"value\"]; expected = json.load(open(\"$evidence_dir/expected-model-roles.json\")); assert actual == expected, (actual, expected)'"
    run_logged models "/opt/homebrew/bin/omp models"
    run_logged unauthenticated-check "/usr/bin/python3 -c 'from pathlib import Path; text = Path(\"$evidence_dir/models.log\").read_text(); assert \"No models available. Set API keys in environment variables.\" in text; print(\"UNVERIFIED: no provider credentials; routes and authenticated models not tested\")'"
    run_logged instructions 'test -f "$HOME/.omp/agent/AGENTS.md"'
    ;;
  mas-signed-out)
    run_logged cli "/opt/homebrew/bin/mas list"
    run_logged metadata "/opt/homebrew/bin/mas info 1037126344"
    run_logged config "/opt/homebrew/bin/mas config"
    run_logged verify-not-installed 'test ! -d "/Applications/Apple Configurator.app"'
    ;;
  optional-modules)
    optional_environments='security,js,development-apps,development-containers,development-editors,development-apple,kubernetes,infrastructure,utilities,messengers,browsers,privacy,productivity,office,design,media,network,admin'
    run_logged preview "cd '$config_dir' && \$HOME/.local/bin/mise -E $optional_environments bootstrap --dry-run"
    run_logged_interactive apply "cd '$config_dir' && \$HOME/.local/bin/mise -E $optional_environments bootstrap --yes"
    run_logged_interactive second-run "cd '$config_dir' && \$HOME/.local/bin/mise -E $optional_environments bootstrap --yes"
    run_logged status "cd '$config_dir' && /bin/zsh -lic 'mise -E $optional_environments bootstrap status'"
    run_logged status-json "/bin/zsh -lic 'mise -E $optional_environments bootstrap status --json' > '$evidence_dir/status.json'"
    run_logged package-verification "/usr/bin/python3 -c 'import json; data = json.load(open(\"$evidence_dir/status.json\")); packages = [package for name in (\"brew\", \"brew-cask\") for package in data[\"packages\"][name][\"packages\"]]; assert all(package[\"state\"] == \"installed\" for package in packages), packages; assert data[\"packages\"][\"mas\"][\"available\"]; print(\"Non-MAS packages installed; MAS applications require sign-in\")'"
    run_logged_interactive cli-verification "/bin/zsh -lic 'mise -E js exec -- node --version && mise -E js exec -- bun --version && kubectl version --client && kubectx --help >/dev/null && helm version && tofu --version && ansible --version && linode-cli --version && gomplate --version && op --version'"
    ;;
  full-dry-run)
    run_logged all-environments "/bin/zsh -lic 'cd /tmp && MISE_CONFIG_DIR=\"$config_dir\" mise -E foundation,terminal,development,security,js,development-rust,development-java,development-apps,development-containers,development-editors,development-apple,kubernetes,infrastructure,utilities,messengers,browsers,privacy,productivity,office,design,media,network,admin,ai-tools,ai-cli,ai-desktop,system-preferences bootstrap --dry-run'"
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
    run_logged stored-state-and-preservation 'set -e
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
