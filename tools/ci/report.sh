#!/bin/sh
# One report per run, from the artifacts every job uploaded.
#
#   sh tools/ci/report.sh <artifacts-dir> > REPORT.md
#
# `download-artifact` with no name puts each artifact in its own directory, so
# the layout is <artifacts-dir>/<artifact>/<the paths the job uploaded>. This
# walks that tree rather than naming paths, so a job changing what it uploads
# does not silently drop a section.
#
# Environment, all optional — the workflow passes them:
#   NEEDS_JSON  `${{ toJSON(needs) }}`, for the status table
#   WORKFLOW GIT_SHA GIT_REF RUN_NUMBER SERVER REPOSITORY IMAGE
set -eu

dir=${1:?usage: report.sh <artifacts-dir>}
[ -d "$dir" ] || { echo "report: no such directory: $dir" >&2; exit 2; }
root=$(cd "$(dirname "$0")/../.." && pwd)

printf '# rust-mf2 — %s report\n\n' "${WORKFLOW:-ci}"

printf '* commit: `%s`\n' "${GIT_SHA:-unknown}"
printf '* ref: `%s`\n' "${GIT_REF:-unknown}"
printf '* run: %s\n' "${RUN_NUMBER:-unknown}"
printf '* generated: %s\n' "$(date -u '+%Y-%m-%d %H:%M:%S UTC')"
printf '* image: `%s`\n' "${IMAGE:-unknown}"
printf '\n'

# ---------------------------------------------------------------- versions
printf '## The tools these figures came from\n\n'
printf 'Every size and every time below was produced by these, on this host.\n'
printf 'A figure is not comparable with one taken elsewhere.\n\n'
printf '```\n'
sh "$root/tools/ci/versions.sh" 2>&1 | sed '/^-\{3,\}/d'
printf '```\n\n'

# ------------------------------------------------------------------- jobs
printf '## Jobs\n\n'
if [ -n "${NEEDS_JSON:-}" ] && command -v jq >/dev/null 2>&1; then
  printf '| job | result |\n|---|---|\n'
  printf '%s' "$NEEDS_JSON" \
    | jq -r 'to_entries | sort_by(.key) | .[] | "| `\(.key)` | \(.value.result) |"'
  printf '\n'
  # `skipped` is not a failure: `release` is skipped on every plain push, by
  # its own `if:`. ci.yml fails the run on this same rule, in the last step of
  # the report job.
  failed=$(printf '%s' "$NEEDS_JSON" | jq -r '[to_entries[] | select(.value.result != "success" and .value.result != "skipped") | .key] | join(", ")')
  if [ -n "$failed" ]; then
    printf '**Not green: %s.** Read that job'"'"'s section below, and its log.\n\n' "$failed"
  else
    printf 'Every job passed, or was skipped.\n\n'
  fi
else
  printf 'No job results were passed to this script (`NEEDS_JSON` empty).\n\n'
fi

# --------------------------------------------------------- what did not run
printf '## Checks that did not run here\n\n'
printf 'The runner has no browser engines, so these are the owner'"'"'s to run\n'
printf 'before a release. This list is `tools/ci/local-only.txt`.\n\n'
printf '| command | what it judges |\n|---|---|\n'
sed -e '/^[[:space:]]*#/d' -e '/^[[:space:]]*$/d' "$root/tools/ci/local-only.txt" \
  | while IFS="$(printf '\t')" read -r cmd what; do
      printf '| `%s` | %s |\n' "$cmd" "$what"
    done
printf '\n'

# ------------------------------------------------------------------ bodies
# Text reports, inlined. Ordered so the ones a release is judged on come first;
# anything not named here still appears, after them, so nothing is lost.
order='REPORT.md size.md report.md feature-costs.md SIZE.md b12.txt size.tsv COVERAGE.md'

# How much of a job's log to inline. The whole thing is in the artifact.
LOG_TAIL=${LOG_TAIL:-120}

listing=$(cd "$dir" && find . -type f | sed 's|^\./||' | sort)

rank() {
  i=0
  for name in $order; do
    i=$((i + 1))
    case "$1" in *"$name") printf '%02d' "$i"; return ;; esac
  done
  printf '99'
}

printf '## Reports\n\n'
echo "$listing" | while read -r rel; do
  [ -n "$rel" ] || continue
  printf '%s\t%s\n' "$(rank "$rel")" "$rel"
done | sort | cut -f2- | while read -r rel; do
  [ -n "$rel" ] || continue
  f="$dir/$rel"
  bytes=$(wc -c < "$f" | tr -d ' ')
  case "$rel" in
    *.log)
      # A job's whole output. The end is where the failure is, so the tail goes
      # in the report and the artifact keeps the rest.
      printf '### `%s`\n\n%s bytes, %s lines. The last %s:\n\n```\n' \
        "$rel" "$bytes" "$(wc -l < "$f" | tr -d ' ')" "$LOG_TAIL lines"
      tail -n "$LOG_TAIL" "$f"
      printf '```\n\n'
      ;;
    *.md|*.txt|*.tsv)
      if [ "$bytes" -gt 262144 ]; then
        printf '### `%s`\n\n%s bytes — too large to inline; it is in the artifact.\n\n' "$rel" "$bytes"
        continue
      fi
      printf '### `%s`\n\n' "$rel"
      case "$rel" in
        # Demote the file's own headings so they nest under this section's
        # `###` instead of competing with the report's own structure. Headings
        # inside a fenced block are left alone.
        *.md) awk '/^```/ { f = !f } { if (!f && /^#/) sub(/^#/, "####"); print }' "$f"
              printf '\n' ;;
        *)    printf '```\n'; cat "$f"; printf '```\n' ;;
      esac
      printf '\n'
      ;;
    *)
      printf '### `%s`\n\n%s bytes — not text; it is in the artifact.\n\n' "$rel" "$bytes"
      ;;
  esac
done

if [ -z "$listing" ]; then
  printf 'No artifacts were downloaded. Every job that should have uploaded one failed\n'
  printf 'before its upload step, or the download step is misconfigured.\n\n'
fi
