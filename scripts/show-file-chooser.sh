#!/usr/bin/env bash
# Shows the running Oxyde's file chooser by calling its portal interface directly, as
# xdg-desktop-portal would, without the System integration option or another application.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: bash scripts/show-file-chooser.sh [--release] [CASE]
Asks the running Oxyde (debug build by default, --release for an installed one)
for a chooser window and prints the portal's answer once it closes.

Cases:
  open         OpenFile, one file (default)
  multiple     OpenFile, several files
  folder       OpenFile with directory: select folders
  filters      OpenFile with type filters (glob and MIME) and a current filter
  choices      OpenFile with a menu and a checkbox in the footer
  save         SaveFile with a suggested name and type filters
  save-again   SaveFile of an existing file (current_file)
  save-files   SaveFiles: a folder for three files
  all          every case, one after the other
EOF
}

destination=io.github.Carlton2001.Oxyde.Devel
cases=()
for argument in "$@"; do
    case "$argument" in
        --release) destination=io.github.Carlton2001.Oxyde ;;
        --help|-h) usage; exit 0 ;;
        *) cases+=("$argument") ;;
    esac
done
destination=${OXYDE_CHOOSER_DESTINATION:-$destination}
[[ ${#cases[@]} -eq 0 ]] && cases=(open)
[[ ${cases[0]} == all ]] && cases=(open multiple folder filters choices save save-again save-files)

home_bytes="b'$HOME'"
filters="[('Images', [(uint32 1, 'image/*')]), ('Text', [(uint32 0, '*.txt'), (uint32 0, '*.md')]), ('All files', [(uint32 0, '*')])]"
choices="[('encoding', 'Encoding', [('utf8', 'UTF-8'), ('latin1', 'Latin-1')], 'utf8'), ('readonly', 'Open read-only', @a(ss) [], 'false')]"

call() {
    local method=$1 title=$2 options=$3
    local handle="/org/freedesktop/portal/desktop/request/oxyde_test/t$$_$RANDOM"
    printf '%s: %s\n' "$method" "$title"
    # The window closes unanswered if this caller leaves the bus, hence the long timeout.
    gdbus call --session --timeout 86400 --dest "$destination" \
        --object-path /org/freedesktop/portal/desktop \
        --method "org.freedesktop.impl.portal.FileChooser.$method" \
        "$handle" io.github.Carlton2001.OxydeTest "" "$title" "$options" || true
}

existing_file() {
    local file
    file=$(find "$HOME" -maxdepth 1 -type f ! -name '.*' -print -quit)
    printf '%s' "${file:-$HOME/.bashrc}"
}

for case_name in "${cases[@]}"; do
    case "$case_name" in
        open) call OpenFile "Open" "{'current_folder': <$home_bytes>}" ;;
        multiple) call OpenFile "Open several" "{'multiple': <true>}" ;;
        folder) call OpenFile "Select folders" "{'directory': <true>, 'multiple': <true>}" ;;
        filters) call OpenFile "Open with filters" "{'filters': <$filters>, 'current_filter': <('Text', [(uint32 0, '*.txt'), (uint32 0, '*.md')])>}" ;;
        choices) call OpenFile "Open with choices" "{'choices': <$choices>, 'accept_label': <'_Import'>}" ;;
        save) call SaveFile "Save as" "{'current_name': <'Notes.txt'>, 'filters': <$filters>, 'current_folder': <$home_bytes>}" ;;
        save-again) call SaveFile "Save again" "{'current_file': <b'$(existing_file)'>}" ;;
        save-files) call SaveFiles "Save files" "{'files': <[b'a.txt', b'b.txt', b'c.txt']>}" ;;
        *) printf 'Unknown case: %s\n\n' "$case_name" >&2; usage >&2; exit 2 ;;
    esac
done
