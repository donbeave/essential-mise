#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/common.sh"

[[ $# -eq 1 ]] || fail "Usage: VM_OWNER_PASSWORD_FILE=<mode-600-file> $0 <guest-command>"
[[ -n "${VM_OWNER_PASSWORD_FILE:-}" && -f "$VM_OWNER_PASSWORD_FILE" && ! -L "$VM_OWNER_PASSWORD_FILE" ]] || \
  fail 'VM_OWNER_PASSWORD_FILE must name a regular, non-symlink credential file'
[[ "$(stat -f '%Lp' "$VM_OWNER_PASSWORD_FILE")" == '600' ]] || \
  fail 'Guest credential file must have mode 600'

vm_require_name
vm_require_running
vm_scan_host_key
VM_AUTO_CONFIRM="${VM_AUTO_CONFIRM:-0}"
[[ "$VM_AUTO_CONFIRM" =~ ^[01]$ ]] || fail 'VM_AUTO_CONFIRM must be 0 or 1'
export VM_IP VM_SSH_USER VM_SSH_KEY VM_KNOWN_HOSTS VM_OWNER_PASSWORD_FILE VM_AUTO_CONFIRM
VM_REMOTE_COMMAND="$1"
export VM_REMOTE_COMMAND

/usr/bin/expect <<'EXPECT'
set timeout 900
log_user 1
set credential_file [open $env(VM_OWNER_PASSWORD_FILE) r]
gets $credential_file owner_password
close $credential_file
if {![info exists owner_password] || $owner_password eq ""} {
  puts stderr "Guest credential file is empty"
  exit 1
}
set command [list /usr/bin/ssh -tt \
  -i $env(VM_SSH_KEY) \
  -o BatchMode=no \
  -o IdentitiesOnly=yes \
  -o PreferredAuthentications=publickey,password \
  -o NumberOfPasswordPrompts=2 \
  -o StrictHostKeyChecking=yes \
  -o UserKnownHostsFile=$env(VM_KNOWN_HOSTS) \
  -o LogLevel=ERROR \
  -o ConnectTimeout=10 \
  "$env(VM_SSH_USER)@$env(VM_IP)" "stty sane; $env(VM_REMOTE_COMMAND)"]
spawn {*}$command
set password_prompts 0
proc clt_ready {command password} {
  log_user 0
  spawn -noecho {*}$command
  set probe $spawn_id
  set pid [exp_pid -i $probe]
  set prompts 0
  set ready 0
  set aborted 0
  set timeout 15
  expect {
    -nocase -re "password:" {
      incr prompts
      if {$prompts > 1} {
        set aborted 1
        catch {exec /bin/kill -KILL $pid}
        catch {close -i $probe}
      } else {
        send -i $probe -- "$password\n"
        exp_continue -continue_timer
      }
    }
    timeout {
      set aborted 1
      catch {exec /bin/kill -KILL $pid}
      catch {close -i $probe}
    }
    eof {}
  }
  if {![catch {wait -i $probe} result] && !$aborted && [llength $result] == 4 && [lindex $result 2] == 0 && [lindex $result 3] == 0} {
    set ready 1
  }
  log_user 1
  return $ready
}
expect {
  -nocase -re "password:" {
    incr password_prompts
    if {$password_prompts > 2} {
      puts stderr "Guest owner authentication failed"
      exit 1
    }
    send -- "$owner_password\n"
    exp_continue
  }
  -re "y/n/enter.*submit" {
    if {$env(VM_AUTO_CONFIRM) eq "1"} {
      send -- "y"
      exp_continue
    }
    puts stderr "Native mise confirmation requires explicit VM_AUTO_CONFIRM=1"
    exit 1
  }
  -re "Finish the Apple installation dialog, then " {
    expect {
      -re {press Return to continue\.\.\. } {}
      timeout { puts stderr "Timed out waiting for Command Line Tools prompt"; exit 124 }
      eof { puts stderr "Guest command exited during Command Line Tools prompt"; exit 1 }
    }
    set deadline [expr {[clock seconds] + 900}]
    set check [list /usr/bin/ssh -T \
      -i $env(VM_SSH_KEY) \
      -o BatchMode=no \
      -o IdentitiesOnly=yes \
      -o PreferredAuthentications=publickey,password \
      -o NumberOfPasswordPrompts=1 \
      -o StrictHostKeyChecking=yes \
      -o UserKnownHostsFile=$env(VM_KNOWN_HOSTS) \
      -o LogLevel=ERROR \
      -o ConnectTimeout=10 \
      "$env(VM_SSH_USER)@$env(VM_IP)" \
      "/usr/bin/xcode-select -p >/dev/null 2>&1 && /usr/bin/git --version >/dev/null 2>&1"]
    while {![clt_ready $check $owner_password]} {
      if {[clock seconds] >= $deadline} {
        puts stderr "Timed out waiting for guest Command Line Tools"
        exit 124
      }
      after 3000
    }
    send -- "\n"
    exp_continue
  }
  -nocase -re "press.*(return|enter)" {
    send -- "\n"
    exp_continue
  }
  timeout {
    puts stderr "Timed out waiting for guest command"
    exit 124
  }
  eof
}
catch wait result
exit [lindex $result 3]
EXPECT
