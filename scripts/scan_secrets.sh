#!/usr/bin/env bash
set -euo pipefail

echo "Running secret scanner on Muxport repository..."

FORBIDDEN_PATTERNS=(
    "sk-[a-zA-Z0-9]{20,}"
    "-----BEGIN PRIVATE KEY-----"
    "-----BEGIN RSA PRIVATE KEY-----"
    "AIzaSy[a-zA-Z0-9_-]{33}"
)

FOUND=0
for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
    if grep -rnE --exclude-dir=".git" --exclude-dir="target" --exclude="scan_secrets.sh" -e "$pattern" . ; then
        echo "ERROR: Potential exposed secret matching pattern '$pattern' found!"
        FOUND=1
    fi
done

if [ $FOUND -eq 0 ]; then
    echo "SUCCESS: Secret scan passed cleanly. No secrets exposed."
    exit 0
else
    echo "FAILED: Secret scan detected exposed patterns."
    exit 1
fi
