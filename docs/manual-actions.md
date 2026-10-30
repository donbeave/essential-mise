# Manual actions and preference reversals

Native mise installs and configures only declared resources. The following remain human decisions: accounts, secrets, licenses, privacy consent, privileged OS changes, private workspaces, and unsupported application configuration.

## Accounts, identity, and credentials

- Set the new machine’s real Git identity deliberately. No `user.name` or `user.email` is deployed.
- Run `gh auth login`, then `gh auth setup-git` for GitHub credential integration. `gh/hosts.yml`, SSH private hosts, and credential-helper reset semantics are not migrated.
- Run `git lfs install` after installing Git LFS.
- Apply the nine nonidentity Git tuning settings and the ignore override with Git itself. Generic INI merging is not used because it can consume indented Git settings as continuations. These commands preserve unrelated keys, repeated helpers, `includeIf`, comments, casing, and mixed indentation:

  ```bash
  config_dir="${MISE_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/mise}"
  git config --global core.autocrlf input
  git config --global core.excludesFile "$config_dir/dotfiles/development/git/ignore"
  git config --global pull.rebase false
  git config --global branch.autoSetupRebase never
  git config --global fetch.prune true
  git config --global merge.conflictStyle zdiff3
  git config --global rerere.enabled true
  git config --global push.autoSetupRemote true
  git config --global init.defaultBranch main
  git config --global alias.merge-main '!git fetch origin && git merge --no-ff origin/main'
  ```

  `core.excludesFile` points to the ignore source in the adopted checkout, so it works with default and custom XDG configuration homes. Review this override before changing or removing it. These commands do not set identity, credential helpers, or SSH commands.
- Sign in to the App Store before applying MAS modules. Purchases/claims and paid apps are not automated. A second apply may be required.
- Sign in to 1Password, enable its Safari extension and SSH agent, and authorize the CLI. The extension itself is installed with `mas:1569813296`.

## Applications and runtimes

- Full Xcode onboarding and licenses are manual; stage 0 installs only Command Line Tools.
- JetBrains Toolbox IDEs/settings, OrbStack VMs, Sketch Library, media servers, and application licenses are manual.
- Browser/VPN/proxy profiles, Surge endpoints/secrets, WARP mode, Little Snitch extension/rules, and privileged network changes are manual.
- Surge uses real Homebrew because mise 2026.10.23 cannot expand the cask binary's `$APPDIR` source. Its hook refuses unmanaged existing Surge applications; migrate those manually. Native mise status, upgrade, and prune do not manage the hook-installed package. See [Homebrew exceptions](homebrew-exceptions.md).
- `agent-browser install` and browser runtime initialization are manual after package installation.
- Amp CLI and OMP use real Homebrew because mise 2026.10.23 leaves Amp's binary non-executable and cannot execute OMP's `with_env` completion helper; see [Homebrew exceptions](homebrew-exceptions.md). Their accounts and provider opt-in are manual. Verify OMP loads `~/.omp/agent/AGENTS.md`, then use `omp config` and `omp models` to validate roles, enabled models, and route overrides; missing authentication is not success.
- OpenCode instructions follow `XDG_CONFIG_HOME`. If `OPENCODE_CONFIG_DIR` is set, OpenCode uses that user-owned directory instead; unset the override or maintain its `AGENTS.md` separately.
- Amp Desktop uses real Homebrew because mise cannot evaluate the cask's `Array#second` DSL. An existing unmanaged `/Applications/Amp.app` must be migrated manually; an installation already owned by the same cask can remain. The package is retained despite a known macOS 27.2 `_dyld_start` GUI stall. A working GUI is not claimed and no security bypass is permitted.
- FFF uses real Homebrew because mise 2026.10.23 installs its raw binary without execute permission. The hook requires `fff-mcp --healthcheck` to pass. Native mise status, upgrade, and prune do not manage the hook-installed package.
- Superwhisper configuration is fully manual. No mode JSON, recordings, or `~/superwhisper` directory is deployed.

## Privacy and privileged host behavior

TCC, SIP, Full Disk Access, Safari protected settings, TIS/HIToolbox input sources, OTP handlers, appearance-key deletion, privileged LaunchDaemons, sysctl/pmset/systemsetup, Gatekeeper, Developer Mode, network time, software-update policy, login-window, MDM, and mobileconfig installation are manual or intentionally not automated.

No fixed Dock icon list or Downloads stack is managed. Symbolic hotkeys are manual: a clean Mac may not have the existing dictionary/parameters, so writing only `enabled` would be incomplete.

## `system-preferences` review and reversal

This module is optional. Review its dry run before applying. Stored-value matching (`MISE_CEILING_PATHS="$config_dir" mise -C "$config_dir" -E system-preferences bootstrap macos defaults status --missing`) is not proof of UI behavior; restart Dock, Finder, SystemUIServer, or the affected application and verify the result on disposable macOS.

Deselecting the module does not undo preferences. Before applying, capture current values with `defaults read DOMAIN KEY` where available. Reversal below uses `defaults delete DOMAIN KEY` to return to macOS defaults or restore the captured previous value. Do not delete an entire dictionary when only one member is managed.

For an exact full-domain rollback, capture every affected domain before applying. These functions work in zsh or Bash and return nonzero on the first unexpected failure. Do not apply preferences unless capture succeeds and prints `capture_dir`:

```bash
capture_preferences() {
  preference_domains=(NSGlobalDomain com.apple.dock com.apple.finder com.apple.spaces com.apple.WindowManager com.apple.assistant.support com.apple.Siri com.apple.screensaver com.apple.onetimepasscodes com.apple.TextInputMenu com.apple.bird)
  if ! capture_dir="$(mktemp -d /tmp/essential-mise-preferences.XXXXXX)"; then
    printf 'Could not create preference snapshot directory\n' >&2
    return 1
  fi
  if [[ -z "$capture_dir" || ! -d "$capture_dir" ]]; then
    printf 'Preference snapshot directory is missing\n' >&2
    return 1
  fi
  local domain state state_file error_file
  for domain in "${preference_domains[@]}"; do
    state_file="$capture_dir/$domain.before.state"
    error_file="$capture_dir/$domain.before.error"
    if defaults read "$domain" >/dev/null 2>&1; then
      if ! defaults export "$domain" "$capture_dir/$domain.before.plist"; then
        printf 'Could not export existing domain %s\n' "$domain" >&2
        return 1
      fi
      printf 'present\n' > "$state_file"
    else
      defaults read "$domain" >/dev/null 2>"$error_file"
      if /usr/bin/grep -F "Domain '$domain' not found." "$error_file" >/dev/null 2>&1; then
        printf 'absent\n' > "$state_file"
      else
        printf 'Could not determine whether domain %s exists:\n' "$domain" >&2
        /bin/cat "$error_file" >&2
        return 1
      fi
    fi
  done
  for domain in "${preference_domains[@]}"; do
    state_file="$capture_dir/$domain.before.state"
    [[ -f "$state_file" ]] || return 1
    IFS= read -r state < "$state_file"
    if [[ "$state" == 'present' && ! -s "$capture_dir/$domain.before.plist" ]]; then
      printf 'Missing snapshot for present domain %s\n' "$domain" >&2
      return 1
    elif [[ "$state" != 'present' && "$state" != 'absent' ]]; then
      printf 'Invalid snapshot state for %s\n' "$domain" >&2
      return 1
    fi
  done
  printf 'capture_dir=%s\n' "$capture_dir"
}
capture_preferences
```

Each present domain has a nonempty plist snapshot. An absent domain is recorded only when macOS explicitly reports it as not found; other read/export errors stop the capture. For an **immediate** rollback, before making any other preference changes, restore the snapshots in the same shell or set `capture_dir` to the printed path:

```bash
restore_preferences() {
  preference_domains=(NSGlobalDomain com.apple.dock com.apple.finder com.apple.spaces com.apple.WindowManager com.apple.assistant.support com.apple.Siri com.apple.screensaver com.apple.onetimepasscodes com.apple.TextInputMenu com.apple.bird)
  local domain state state_file error_file
  [[ -n "${capture_dir:-}" ]] || return 1
  for domain in "${preference_domains[@]}"; do
    state_file="$capture_dir/$domain.before.state"
    [[ -f "$state_file" ]] || return 1
    IFS= read -r state < "$state_file"
    case "$state" in
      present)
        [[ -s "$capture_dir/$domain.before.plist" ]] || return 1
        if ! defaults delete "$domain"; then
          printf 'Could not clear domain %s before restoring its snapshot\n' "$domain" >&2
          return 1
        fi
        if ! defaults import "$domain" "$capture_dir/$domain.before.plist"; then
          printf 'Could not restore domain %s\n' "$domain" >&2
          return 1
        fi
        ;;
      absent)
        if defaults read "$domain" >/dev/null 2>&1; then
          if ! defaults delete "$domain"; then
            printf 'Could not remove newly created domain %s\n' "$domain" >&2
            return 1
          fi
        else
          error_file="$capture_dir/$domain.restore.error"
          defaults read "$domain" >/dev/null 2>"$error_file"
          if ! /usr/bin/grep -F "Domain '$domain' not found." "$error_file" >/dev/null 2>&1; then
            printf 'Could not confirm that domain %s is absent:\n' "$domain" >&2
            /bin/cat "$error_file" >&2
            return 1
          fi
        fi
        ;;
      *)
        printf 'Invalid snapshot state for %s\n' "$domain" >&2
        return 1
        ;;
    esac
  done
}
restore_preferences
```

Restore also returns nonzero on the first unexpected failure; fix the reported problem and rerun it rather than assuming every domain was restored.

Deleting and reimporting a present domain replaces its complete contents and removes keys added after capture; never use full-domain restoration as a delayed or routine cleanup operation. For later selective reversal, use the per-key commands below. For the two nested values, use `plutil -extract AppleFirstWeekday.gregorian raw -o - "$capture_dir/NSGlobalDomain.before.plist"` and `plutil -extract AppleICUDateFormatStrings.1 raw -o - "$capture_dir/NSGlobalDomain.before.plist"` to inspect the original members, then restore only those members with `defaults write NSGlobalDomain AppleFirstWeekday -dict-add gregorian -int ORIGINAL_INTEGER` or `defaults write NSGlobalDomain AppleICUDateFormatStrings -dict-add 1 -string ORIGINAL_STRING`. If the original member was absent, use the immediate snapshot rollback instead of deleting the whole dictionary.

Observable UI behavior and delayed selective restoration remain manual validation boundaries. Restart the affected process and verify the actual interaction: `killall Dock` for Dock/hot-corner values, `killall Finder` for Finder values, and `killall SystemUIServer` for menu/Space-related values. For locale, language, text-input, keyboard, screensaver, storage, Siri, and Window Manager behavior, log out and back in or restart the affected application before testing. Do not claim behavior from stored values alone.

### Dock and keyboard

| Preference | Effect | Reversal |
|---|---|---|
| `com.apple.dock autohide` | Hides Dock automatically | `defaults delete com.apple.dock autohide` |
| `com.apple.dock orientation` | Pins Dock to bottom | Restore previous orientation or `defaults delete com.apple.dock orientation` |
| `com.apple.dock tilesize` | Sets icon size to 58 | Restore previous size or `defaults delete com.apple.dock tilesize` |
| `com.apple.dock magnification` | Disables magnification | Restore previous setting or `defaults delete com.apple.dock magnification` |
| `com.apple.dock show-recents` | Hides recent applications | Restore previous setting or `defaults delete com.apple.dock show-recents` |
| `com.apple.dock mru-spaces` | Disables most-recent-use Spaces ordering | Restore previous setting or `defaults delete com.apple.dock mru-spaces` |
| `NSGlobalDomain KeyRepeat` | Repeat rate 1 | Restore previous rate or `defaults delete NSGlobalDomain KeyRepeat` |
| `NSGlobalDomain InitialKeyRepeat` | Initial delay 10 | Restore previous delay or `defaults delete NSGlobalDomain InitialKeyRepeat` |
| `NSGlobalDomain ApplePressAndHoldEnabled` | Repeats held keys instead of accents | `defaults delete NSGlobalDomain ApplePressAndHoldEnabled` |

### Finder, Dock raw values, Spaces, and Window Manager

| Preference | Effect | Reversal |
|---|---|---|
| `com.apple.finder FXRemoveOldTrashItems` | Permanently removes Trash items after 30 days | `defaults delete com.apple.finder FXRemoveOldTrashItems`; review Trash before disabling |
| `com.apple.finder CreateDesktop` | Shows Desktop icons | `defaults delete com.apple.finder CreateDesktop` |
| `com.apple.dock mineffect` | Genie minimize effect | `defaults delete com.apple.dock mineffect` |
| `com.apple.dock minimize-to-application` | Does not minimize into app icon | `defaults delete com.apple.dock minimize-to-application` |
| `com.apple.dock launchanim` | Enables launch animation | `defaults delete com.apple.dock launchanim` |
| `com.apple.dock show-process-indicators` | Shows running-app indicators | `defaults delete com.apple.dock show-process-indicators` |
| `com.apple.dock workspaces-auto-swoosh` | Disables automatic switching to a Space containing an application's windows | `defaults delete com.apple.dock workspaces-auto-swoosh` |
| `com.apple.dock expose-group-apps` | Disables app grouping in Mission Control | `defaults delete com.apple.dock expose-group-apps` |
| `com.apple.dock enter-mission-control-by-top-window-drag` | Disables top-window drag into Mission Control | `defaults delete com.apple.dock enter-mission-control-by-top-window-drag` |
| `com.apple.dock wvous-tl-corner` | Top-left hot corner off | Restore previous hot corner or `defaults delete com.apple.dock wvous-tl-corner` |
| `com.apple.dock wvous-tr-corner` | Top-right hot corner Mission Control | Restore previous hot corner or `defaults delete com.apple.dock wvous-tr-corner` |
| `com.apple.dock wvous-bl-corner` | Bottom-left hot corner Desktop | Restore previous hot corner or `defaults delete com.apple.dock wvous-bl-corner` |
| `com.apple.dock wvous-br-corner` | Bottom-right hot corner application windows | Restore previous hot corner or `defaults delete com.apple.dock wvous-br-corner` |
| `com.apple.dock wvous-{tl,tr,bl,br}-modifier` | Hot-corner modifiers cleared | Restore each previous modifier or delete the corresponding key |
| `com.apple.spaces spans-displays` | Displays do not span Spaces | `defaults delete com.apple.spaces spans-displays` |
| `com.apple.WindowManager GloballyEnabled` | Disables Stage Manager | `defaults delete com.apple.WindowManager GloballyEnabled` |
| `com.apple.WindowManager EnableStandardClickToShowDesktop` | Enables standard click-to-show-Desktop | `defaults delete com.apple.WindowManager EnableStandardClickToShowDesktop` |
| `com.apple.WindowManager EnableTilingByEdgeDrag` | Disables edge-drag tiling | `defaults delete com.apple.WindowManager EnableTilingByEdgeDrag` |
| `com.apple.WindowManager EnableTopTilingByEdgeDrag` | Disables top-edge tiling | `defaults delete com.apple.WindowManager EnableTopTilingByEdgeDrag` |
| `com.apple.WindowManager EnableTilingOptionAccelerator` | Disables tiling Option accelerator | `defaults delete com.apple.WindowManager EnableTilingOptionAccelerator` |

### Global appearance, locale, text, and application behavior

| Preference | Effect | Reversal |
|---|---|---|
| `AppleInterfaceStyleSwitchesAutomatically` | Automatic light/dark appearance | `defaults delete NSGlobalDomain AppleInterfaceStyleSwitchesAutomatically` |
| `AppleLocale` | Region `en_US` | Restore previous locale or `defaults delete NSGlobalDomain AppleLocale` |
| `AppleMetricUnits` | Metric measurements | Restore previous setting or `defaults delete NSGlobalDomain AppleMetricUnits` |
| `AppleMeasurementUnits` | Centimeters | Restore previous unit or `defaults delete NSGlobalDomain AppleMeasurementUnits` |
| `AppleTemperatureUnit` | Celsius | Restore previous unit or `defaults delete NSGlobalDomain AppleTemperatureUnit` |
| `AppleICUForce24HourTime` | 24-hour time | `defaults delete NSGlobalDomain AppleICUForce24HourTime` |
| `AppleLiveTextEnabled` | Enables Live Text | `defaults delete NSGlobalDomain AppleLiveTextEnabled` |
| `AppleActionOnDoubleClick` | Double-click maximizes | `defaults delete NSGlobalDomain AppleActionOnDoubleClick` |
| `AppleMiniaturizeOnDoubleClick` | Disables double-click minimize | `defaults delete NSGlobalDomain AppleMiniaturizeOnDoubleClick` |
| `AppleWindowTabbingMode` | Fullscreen window tabs | `defaults delete NSGlobalDomain AppleWindowTabbingMode` |
| `NSCloseAlwaysConfirmsChanges` | No close confirmation by default | `defaults delete NSGlobalDomain NSCloseAlwaysConfirmsChanges` |
| `NSQuitAlwaysKeepsWindows` | Does not restore windows on quit | `defaults delete NSGlobalDomain NSQuitAlwaysKeepsWindows` |
| `NSAutomaticSpellingCorrectionEnabled` | Disables spelling correction | `defaults delete NSGlobalDomain NSAutomaticSpellingCorrectionEnabled` |
| `NSAutomaticCapitalizationEnabled` | Disables automatic capitalization | `defaults delete NSGlobalDomain NSAutomaticCapitalizationEnabled` |
| `NSAutomaticInlinePredictionEnabled` | Disables inline predictions | `defaults delete NSGlobalDomain NSAutomaticInlinePredictionEnabled` |
| `NSSmartReplyEnabled` | Enables suggested replies | `defaults delete NSGlobalDomain NSSmartReplyEnabled` |
| `NSAutomaticPeriodSubstitutionEnabled` | Disables period substitution | `defaults delete NSGlobalDomain NSAutomaticPeriodSubstitutionEnabled` |
| `NSAutomaticQuoteSubstitutionEnabled` | Disables quote substitution | `defaults delete NSGlobalDomain NSAutomaticQuoteSubstitutionEnabled` |
| `NSAutomaticDashSubstitutionEnabled` | Disables dash substitution | `defaults delete NSGlobalDomain NSAutomaticDashSubstitutionEnabled` |
| `KB_DoubleQuoteOption` | Double-quote style `“abc”` | `defaults delete NSGlobalDomain KB_DoubleQuoteOption` |
| `KB_SingleQuoteOption` | Single-quote style `‘abc’` | `defaults delete NSGlobalDomain KB_SingleQuoteOption` |
| `NSTableViewDefaultSizeMode` | Medium sidebar/icon size | `defaults delete NSGlobalDomain NSTableViewDefaultSizeMode` |
| `AppleReduceDesktopTinting` | Disables desktop tinting reduction | `defaults delete NSGlobalDomain AppleReduceDesktopTinting` |
| `AppleShowScrollBars` | Automatic scroll bars | `defaults delete NSGlobalDomain AppleShowScrollBars` |
| `AppleLanguages` | Language list only `en-US` | Restore previous language list or `defaults delete NSGlobalDomain AppleLanguages` |

### Nested date/locale values

Only the named dictionary member is managed; unrelated members must remain.

| Preference | Effect | Reversal |
|---|---|---|
| `AppleFirstWeekday.gregorian` | Week starts Monday (`2`) | Restore only the captured `gregorian` member as described above; do not replace the dictionary or unrelated calendar members |
| `AppleICUDateFormatStrings.1` | Short date `y-MM-dd` | Restore only the captured `1` member as described above; do not replace the dictionary or unrelated format members |

### Assistant, security, storage, and text input

| Preference | Effect | Reversal |
|---|---|---|
| `com.apple.assistant.support Assistant Enabled` | Disables assistant support | `defaults delete com.apple.assistant.support 'Assistant Enabled'` |
| `com.apple.Siri StatusMenuVisible` | Hides Siri menu item | `defaults delete com.apple.Siri StatusMenuVisible` |
| `com.apple.Siri VoiceTriggerUserEnabled` | Disables Siri voice trigger | `defaults delete com.apple.Siri VoiceTriggerUserEnabled` |
| `com.apple.screensaver askForPassword` | Requires password after sleep/screensaver | `defaults delete com.apple.screensaver askForPassword` |
| `com.apple.screensaver askForPasswordDelay` | Five-minute grace period | `defaults delete com.apple.screensaver askForPasswordDelay` |
| `com.apple.onetimepasscodes DeleteVerificationCodes` | Deletes used verification codes | `defaults delete com.apple.onetimepasscodes DeleteVerificationCodes`; review before disabling |
| `com.apple.TextInputMenu visible` | Shows text input menu | `defaults delete com.apple.TextInputMenu visible` |
| `com.apple.bird optimize-storage` | Enables storage optimization and can affect local availability | `defaults delete com.apple.bird optimize-storage`; review file availability before changing |
