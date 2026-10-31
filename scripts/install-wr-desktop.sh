#!/usr/bin/env bash
# Register only Wireroom for the current user, preserving different prior entries.
set -euo pipefail
root=$(git -C "$(dirname -- "$0")" rev-parse --show-toplevel)
executable=$(realpath -- "${1:-$root/target/debug/wr}")
if [[ ! -x "$executable" || "$executable" == *$'\n'* ]]; then
    echo "Expected the path to an executable wr binary" >&2
    exit 1
fi
data=${XDG_DATA_HOME:-$HOME/.local/share}
entry="$data/applications/io.github.o_kos.wireroom.desktop"
icon="$data/icons/hicolor/scalable/apps/io.github.o_kos.wireroom.svg"
if [[ -e "$entry" ]] && ! grep -qx 'Name=Wireroom' "$entry"; then
    echo "Refusing to replace an unrelated desktop entry" >&2
    exit 1
fi
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
# Desktop Exec quoting has its own escaping, including literal field-code percent.
escaped=${executable//\\/\\\\}
escaped=${escaped//\"/\\\"}
escaped=${escaped//\$/\\\$}
escaped=${escaped//\`/\\\`}
escaped=${escaped//%/%%}
while IFS= read -r line; do
    if [[ "$line" == Exec=* ]]; then
        printf 'Exec="%s"\n' "$escaped"
    else
        printf '%s\n' "$line"
    fi
done < "$root/crates/ui/assets/io.github.o_kos.wireroom.desktop" > "$work/wireroom.desktop"
for destination in "$entry" "$icon"; do
    source="$work/wireroom.desktop"
    if [[ "$destination" == "$icon" ]]; then source="$root/assets/icons/wireroom.svg"; fi
    if [[ -e "$destination" ]] && ! cmp -s "$source" "$destination"; then
        backup=$(mktemp "${destination}.backup.XXXXXX")
        cp -p -- "$destination" "$backup"
        printf 'Preserved previous Wireroom artifact: %s\n' "$backup"
    fi
    install -Dm644 "$source" "$destination"
done
if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$data/applications"
fi
if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache --force --ignore-theme-index "$data/icons/hicolor"
fi
printf '%s\n' 'Registered Wireroom for the current user.'
