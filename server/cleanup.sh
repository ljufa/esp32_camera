#!/bin/sh
RETAIN_DAYS=${RETAIN_DAYS:-7}
SAVE_DIR=${SAVE_DIR:-/frames}

echo "[cleanup] $(date '+%Y-%m-%d %H:%M:%S') start — retain_days=${RETAIN_DAYS} save_dir=${SAVE_DIR}"

jpg_deleted=0
while IFS= read -r f; do
    [ -z "$f" ] && continue
    echo "[cleanup]   deleting: $f"
    rm -f "$f"
    jpg_deleted=$((jpg_deleted + 1))
done << EOF
$(find "$SAVE_DIR" -name "*.jpg" -mtime +"$RETAIN_DAYS" 2>/dev/null)
EOF
echo "[cleanup] deleted ${jpg_deleted} .jpg file(s)"

dirs_removed=0
while IFS= read -r d; do
    [ -z "$d" ] && continue
    echo "[cleanup]   removing empty dir: $d"
    rmdir "$d"
    dirs_removed=$((dirs_removed + 1))
done << EOF
$(find "$SAVE_DIR" -mindepth 2 -depth -type d -empty 2>/dev/null)
EOF
echo "[cleanup] removed ${dirs_removed} empty director(ies)"

echo "[cleanup] $(date '+%Y-%m-%d %H:%M:%S') done"
