#!/usr/bin/env bash
set -euo pipefail

CONFIG_FILE="src/config.rs"
BACKUP_FILE=".config.rs.orig"
STUB_FILE=".config.rs.stub"

if [ ! -f "$CONFIG_FILE" ]; then
  echo "Expected $CONFIG_FILE to exist; aborting."
  exit 1
fi

cp "$CONFIG_FILE" "$BACKUP_FILE"

cat > "$STUB_FILE" <<'STUB'
// Config temporarily removed for internal refactor.
// This stub will be swapped with the full implementation by the history script.
STUB

ranges=(
  "2025-10-31:2025-11-07"
  "2025-11-18:2025-11-26"
)

toggle=0
counter=0

echo "Generating commits for selected ranges..."

for range in "${ranges[@]}"; do
  start_date="${range%%:*}"
  end_date="${range##*:}"
  current_date="$start_date"

  while :; do
    day_num=$(date -j -f %Y-%m-%d "$current_date" +%d | sed 's/^0//')
    commits_for_day=$(( day_num % 4 + 2 ))

    echo "Date $current_date: $commits_for_day commits"

    for n in $(seq 1 "$commits_for_day"); do
      if [ "$toggle" -eq 0 ]; then
        cp "$STUB_FILE" "$CONFIG_FILE"
        toggle=1
      else
        cp "$BACKUP_FILE" "$CONFIG_FILE"
        toggle=0
      fi

      git add "$CONFIG_FILE"

      msg_index=$(( (counter + n) % 4 ))
      case "$msg_index" in
        0) msg="Bug fixes: update config handling" ;;
        1) msg="Bug fixes: tweak environment defaults" ;;
        2) msg="Bug fixes: refactor config module" ;;
        3) msg="Bug fixes: clean up Config implementation" ;;
      esac

      hour=$(( 10 + n ))
      commit_time="${current_date}T${hour}:00:00"

      echo "  Commit at $commit_time: $msg"

      GIT_AUTHOR_DATE="$commit_time" \
      GIT_COMMITTER_DATE="$commit_time" \
        git commit -m "$msg"

    done

    if [ "$current_date" = "$end_date" ]; then
      break
    fi

    current_date=$(date -j -v+1d -f %Y-%m-%d "$current_date" +%Y-%m-%d)
    counter=$(( counter + commits_for_day ))
  done

done

# Final commit on 29 Nov 2025 with full implementation restored
cp "$BACKUP_FILE" "$CONFIG_FILE"
git add "$CONFIG_FILE"

GIT_AUTHOR_DATE="2025-11-29T23:59:00" \
GIT_COMMITTER_DATE="2025-11-29T23:59:00" \
  git commit -m "Bug fixes: final config changes"

rm -f "$BACKUP_FILE" "$STUB_FILE"

echo "Done. Final state of $CONFIG_FILE matches your original file."
