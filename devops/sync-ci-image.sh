#!/usr/bin/env bash
# Copy the CI toolchain image into a public package, so every job can pull it
# without a secret.
#
#   bash devops/sync-ci-image.sh [--source REF] [--dest REF] [--dry-run]
#
# Why this exists. The image the other Rust repositories here build with lives
# at git.coworkunion.com/coworkunion/chattyness/leptos-builder, and the
# `coworkunion` organisation is private. Forgejo has no per-package visibility
# setting — a package is as visible as its owner — so that image cannot be made
# public while the organisation is not. Checked on this instance (Forgejo
# 15.0.3, 2026-10-06): /api/v1/orgs/coworkunion is 404 and
# /api/v1/packages/coworkunion is 403 anonymously, while
# /api/v1/packages/EvanCarroll is 200.
#
# `EvanCarroll` is a public user and already publishes a container package
# (forgejo-podman-runner), which can be pulled with an anonymous registry token.
# So a copy under that owner is world-readable, and mf2's jobs need no
# `credentials:` block and no REGISTRY_TOKEN at all. Without that, every job
# fails before its first step with
#
#   failed to handle credentials: failed to interpolate container.credentials.password
#
# which is act_runner refusing an empty secret.
#
# What it guards. Copying `:latest` copies whatever is there, so a source image
# older than the tools the checks need would publish silently and every job
# would then fail in setup.sh instead. This refuses to push an image that does
# not carry the whole list tools/ci/setup.sh verifies.
#
# Needs: podman, and `podman login git.coworkunion.com` first (a token with
# read on the source's owner and write on the destination's). Nothing here
# stores a credential.
set -euo pipefail

SOURCE=git.coworkunion.com/coworkunion/chattyness/leptos-builder:latest
DEST=git.coworkunion.com/evancarroll/mf2-ci:latest
dry=no

while [ $# -gt 0 ]; do
  case $1 in
    --source) SOURCE=${2:?--source needs a reference}; shift 2 ;;
    --dest)   DEST=${2:?--dest needs a reference}; shift 2 ;;
    --dry-run) dry=yes; shift ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

host=${DEST%%/*}

say() { printf '\n== %s\n' "$*"; }

command -v podman >/dev/null 2>&1 || { echo "podman is not installed" >&2; exit 2; }

say "Checking the login for $host"
if ! podman login --get-login "$host" >/dev/null 2>&1; then
  cat >&2 <<EOF
Not logged in to $host.

  podman login $host

Use a token that can read ${SOURCE%%/*}'s packages and write the destination
owner's. This script stores nothing.
EOF
  exit 1
fi
echo "logged in as $(podman login --get-login "$host")"

say "Pulling $SOURCE"
podman pull "$SOURCE"

# The list tools/ci/setup.sh verifies. Keep the two in step: setup.sh failing
# in CI and this failing here should mean the same thing.
TOOLS="bash xmllint brotli nm wasm-opt wasm-dis node jq cargo rustup"

say "Checking the image carries what the checks need"
missing=$(podman run --rm --entrypoint "" "$SOURCE" sh -c '
  m=
  for t in '"$TOOLS"'; do command -v "$t" >/dev/null 2>&1 || m="$m $t"; done
  printf "%s" "$m"
')
if [ -n "$missing" ]; then
  cat >&2 <<EOF

$SOURCE is missing:$missing

It is older than the tools the checks need. Rebuild it first — bash and
libxml2-utils were added to leptos-builder's Containerfile on 2026-10-05 — and
run this again. Publishing it now would only move the failure into every job's
setup step.
EOF
  exit 1
fi
echo "every tool present:$(printf ' %s' $TOOLS)"

digest=$(podman image inspect --format '{{.Digest}}' "$SOURCE")
stamp=$(date -u +%Y%m%d%H%M%S)
echo "source digest: $digest"

say "Tagging"
# A moving tag for the workflows, and two fixed ones so a bad image can be
# rolled back to a known one.
for tag in "$DEST" "${DEST%:*}:$stamp" "${DEST%:*}:${digest#sha256:}"; do
  echo "  $tag"
  [ "$dry" = yes ] || podman tag "$SOURCE" "$tag"
done

if [ "$dry" = yes ]; then
  say "Dry run: nothing pushed"
  exit 0
fi

say "Pushing"
for tag in "$DEST" "${DEST%:*}:$stamp" "${DEST%:*}:${digest#sha256:}"; do
  echo "  $tag"
  podman push "$tag"
done

say "Checking it is readable without a credential"
# The whole point: an anonymous token must be enough. A registry always answers
# the first request with 401 and names its token endpoint, so the handshake —
# not that 401 — is what says whether the package is public.
repo=${DEST#*/}; repo=${repo%:*}
token=$(curl -sS --max-time 20 \
  "https://$host/v2/token?service=container_registry&scope=repository:$repo:pull" \
  | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')
if [ -z "$token" ]; then
  echo "no anonymous token for $repo — the destination owner may not be public" >&2
  exit 1
fi
code=$(curl -sS --max-time 20 -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $token" \
  -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.docker.distribution.manifest.list.v2+json' \
  "https://$host/v2/$repo/manifests/${DEST##*:}")
if [ "$code" != 200 ]; then
  cat >&2 <<EOF
Anonymous read of $DEST returned HTTP $code, not 200.

The destination owner is not public, so the workflows still need a credential.
Check that ${DEST#*/} belongs to a public user or organisation.
EOF
  exit 1
fi

say "Done"
cat <<EOF
$DEST is public: an anonymous pull returns HTTP 200.

The workflows pull it with no \`credentials:\` block and no REGISTRY_TOKEN.
Rolled-back tags, if one is ever needed:
  ${DEST%:*}:$stamp
  ${DEST%:*}:${digest#sha256:}
EOF
