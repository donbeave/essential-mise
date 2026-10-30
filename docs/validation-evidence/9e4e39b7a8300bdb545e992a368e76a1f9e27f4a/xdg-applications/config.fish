# synthetic user setting
set -g SYNTHETIC_FISH_VALUE preserve

# >>> mise:activate >>> managed by mise — do not edit between markers
if status is-interactive
    mise activate fish | source
else
    mise activate fish --shims | source
end
# <<< mise:activate <<<

# >>> mise:amp >>> managed by mise — do not edit between markers
if status is-interactive
    command -q amp; and amp completions fish | source
end
# <<< mise:amp <<<
