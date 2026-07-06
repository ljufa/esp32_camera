#!/usr/bin/env bash
# Publish a sanitized snapshot of main to the public remote.
#
# The public branch (main_public) intentionally shares no history with main:
# main's history contains credentials and firmware binaries with credentials
# baked in, so it can never be pushed publicly. Each publish adds one squash
# commit with the sanitized tree.
#
# Usage: ./publish_public.sh          # prepare + commit locally, show diff
#        ./publish_public.sh --push   # also push to $REMOTE
set -euo pipefail
cd "$(dirname "$0")"

REMOTE=origin_public
BRANCH=main_public
KCONFIG=firmware/main/Kconfig.projbuild

# Tracked paths that must never be published.
EXCLUDE_PATHS=(
    firmware/release
    server/.env
)

# Kconfig options whose defaults hold real credentials/URLs; the public
# copy gets these placeholders instead.
declare -A PLACEHOLDER=(
    [WIFI_SSID]="myssid"
    [WIFI_PASSWORD]="mypassword"
    [HTTP_ENDPOINT_URL]="http://192.168.1.100:8080/upload"
    [OTA_FIRMWARE_URL]="http://192.168.1.100:8080/firmware"
    [HTTP_BASIC_AUTH_USERNAME]="camera"
    [HTTP_BASIC_AUTH_PASSWORD]="changeme"
    [OTA_BASIC_AUTH_USERNAME]="camera"
    [OTA_BASIC_AUTH_PASSWORD]="changeme"
)

[ -z "$(git status --porcelain)" ] || { echo "ERROR: working tree not clean"; exit 1; }
SRC_SHA=$(git rev-parse --short main)

# Collect the real secret values (from the private tree) for the leak check.
# Usernames are placeholdered above but skipped here: they collide with
# legitimate example names (e.g. "camera_ota" in .env.example).
SECRETS=()
for key in "${!PLACEHOLDER[@]}"; do
    case "$key" in *_USERNAME) continue ;; esac
    val=$(sed -n "/config ${key}\$/,/help/ s/^[[:space:]]*default \"\(.*\)\"/\1/p" "$KCONFIG" | head -1)
    if [ -n "$val" ] && [ "$val" != "${PLACEHOLDER[$key]}" ]; then
        SECRETS+=("$val")
    fi
done
while IFS='=' read -r k v; do
    [ -n "$v" ] && SECRETS+=("$v")
done < <(grep -E '^[A-Z_]+=' server/.env | grep -v '^PUBLIC_')

# Build the sanitized tree from tracked files only.
STAGE=$(mktemp -d)
WT=""
trap 'rm -rf "$STAGE"; [ -n "$WT" ] && git worktree remove --force "$WT" 2>/dev/null || true' EXIT
git archive main | tar -x -C "$STAGE"

for p in "${EXCLUDE_PATHS[@]}"; do
    rm -rf "${STAGE:?}/${p}"
done

for key in "${!PLACEHOLDER[@]}"; do
    sed -i "/config ${key}\$/,/help/ s|^\([[:space:]]*default \)\".*\"|\1\"${PLACEHOLDER[$key]}\"|" \
        "$STAGE/$KCONFIG"
done

# Leak check: none of the real values may appear anywhere in the snapshot.
for s in "${SECRETS[@]}"; do
    if grep -rIq -F -- "$s" "$STAGE"; then
        echo "ERROR: secret value still present in sanitized tree:"
        grep -rIl -F -- "$s" "$STAGE"
        exit 1
    fi
done
if find "$STAGE" -name '*.bin' | grep -q .; then
    echo "ERROR: binary files in sanitized tree:"; find "$STAGE" -name '*.bin'
    exit 1
fi

# Commit the snapshot onto the public branch via a temporary worktree.
WT=$(mktemp -d -u)
git worktree add "$WT" "$BRANCH"
find "$WT" -mindepth 1 -maxdepth 1 -not -name .git -exec rm -rf {} +
cp -a "$STAGE"/. "$WT"/
git -C "$WT" add -A
if git -C "$WT" diff --cached --quiet; then
    echo "Nothing to publish: $BRANCH already matches main ($SRC_SHA)"
else
    git -C "$WT" commit -m "sync from private main ($SRC_SHA)"
    git -C "$WT" show --stat --format="%h %s" HEAD
fi

if [ "${1:-}" = "--push" ]; then
    git push "$REMOTE" "$BRANCH:main"
else
    echo
    echo "Review with: git diff ${BRANCH}~1 ${BRANCH}"
    echo "Publish with: git push $REMOTE $BRANCH:main"
fi
