# Manual actions and preference reversals

Native mise installs and configures only declared resources. The following remain human decisions: accounts, secrets, licenses, privacy consent, privileged OS changes, private workspaces, and unsupported application configuration.

## Accounts, identity, and credentials

- Set the new machine’s real Git identity deliberately. The three global Git settings used during implementation are not copied as a dotfile identity.
- Run `gh auth login`, then `gh auth setup-git` for GitHub credential integration. `gh/hosts.yml`, SSH private hosts, and credential-helper reset semantics are not migrated.
- Run `git lfs install` after installing Git LFS.
- Sign in to the App Store before applying MAS modules. Purchases/claims and paid apps are not automated. A second apply may be required.
- Sign in to 1Password, enable its Safari extension and SSH agent, and authorize the CLI. The extension itself is installed with `mas:1569813296`.

## Applications and runtimes

- Full Xcode onboarding and licenses are manual; stage 0 installs only Command Line Tools.
- JetBrains Toolbox IDEs/settings, OrbStack VMs, Sketch Library, media servers, and application licenses are manual.
- Browser/VPN/proxy profiles, Surge endpoints/secrets, WARP mode, Little Snitch extension/rules, and privileged network changes are manual.
- `agent-browser install` and browser runtime initialization are manual after package installation.
- Amp CLI and OMP use real Homebrew because mise 2026.10.23 leaves Amp's binary non-executable and cannot execute OMP's `with_env` completion helper; see [Homebrew exceptions](homebrew-exceptions.md). Their accounts and any provider opt-in are manual. Verify OMP loads `~/.omp/agent/AGENTS.md`, then use `omp config` and `omp models` to validate roles, enabled models, and route overrides; missing authentication is not success. Drop stale settings.
- Amp Desktop uses real Homebrew because mise cannot evaluate the cask's `Array#second` DSL. For an existing unmanaged `/Applications/Amp.app`, the module refuses to continue until you migrate ownership manually. An existing installation already owned by the same Homebrew cask can remain. Two independent guests reproduced Amp's `_dyld_start` GUI stall, so GUI setup is MANUAL/DROP on macOS 27.2; package installation does not establish a usable desktop app. Release tests must cover clean installation and existing-app conflict handling without automatic adoption.
- FFF uses real Homebrew because mise 2026.10.23 installs its raw binary without execute permission. The native tap was removed; an idempotent Homebrew hook installs it and requires `fff-mcp --healthcheck` to pass. Native mise status, upgrade, and prune are not used to manage these hook-installed packages.
- Superwhisper configuration is fully manual. No mode JSON, recordings, or `~/superwhisper` directory is deployed.

## Privacy and privileged host behavior

TCC, SIP, Full Disk Access, Safari protected settings, TIS/HIToolbox input sources, OTP handlers, appearance-key deletion, privileged LaunchDaemons, sysctl/pmset/systemsetup, Gatekeeper, Developer Mode, network time, software-update policy, login-window, MDM, and mobileconfig installation are manual or intentionally dropped.

No fixed Dock icon list or Downloads stack is managed. Symbolic hotkeys are manual: a clean Mac may not have the existing dictionary/parameters, so writing only `enabled` would be incomplete.

## `system-preferences` review and reversal

This module is optional. Review its dry run before applying. Stored-value matching (`mise bootstrap macos defaults status --missing`) is not proof of UI behavior; restart Dock, Finder, SystemUIServer, or the affected application and verify the result on disposable macOS.

Deselecting the module does not undo preferences. Before applying, capture current values with `defaults read DOMAIN KEY` where available. Reversal below uses `defaults delete DOMAIN KEY` to return to macOS defaults or restore the captured previous value. Do not delete an entire dictionary when only one member is managed.

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
| `com.apple.dock workspaces-auto-swoosh` | Disables automatic Space switching animation | `defaults delete com.apple.dock workspaces-auto-swoosh` |
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
| `AppleFirstWeekday.gregorian` | Week starts Monday (`2`) | Restore the captured complete `AppleFirstWeekday` value; do not delete unrelated calendar members |
| `AppleICUDateFormatStrings.1` | Short date `y-MM-dd` | Restore the captured complete `AppleICUDateFormatStrings` value; do not delete unrelated format members |

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
