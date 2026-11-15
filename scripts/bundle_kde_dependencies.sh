#!/usr/bin/env bash
# Complete the KDE library closure without leaving dangling SONAME symlinks.
set -eu

appdir=${1:?usage: bundle_kde_dependencies.sh APPDIR [LIBRARY_DIR]}
library_dir=${2:-/usr/lib64}
destination="$appdir/usr/lib"
mkdir -p "$destination"

added=1
while [ "$added" -ne 0 ]; do
    added=0
    deps=$(find "$appdir" -type f \( -name '*.so*' -o -perm /111 \) -print0 |
        xargs -0 -r ldd 2>/dev/null |
        awk '$2 == "=>" && $1 ~ /^(libKF6|libKirigami)/ { print $1 }' |
        sort -u)
    for dep in $deps; do
        if [ ! -f "$destination/$dep" ] && [ ! -f "$appdir/usr/lib64/$dep" ]; then
            echo "Bundling missing dependency: $dep"
            # The SONAME's target may have a different version prefix. Copy its
            # bytes under the SONAME, replacing any existing dangling link.
            cp -L --remove-destination -- "$library_dir/$dep" "$destination/$dep"
            if [ ! -f "$destination/$dep" ]; then
                echo "Failed to bundle dependency: $dep" >&2
                exit 1
            fi
            added=1
        fi
    done
done
