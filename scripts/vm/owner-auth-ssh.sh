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
export VM_IP VM_SSH_USER VM_SSH_KEY VM_KNOWN_HOSTS VM_OWNER_PASSWORD_FILE
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
