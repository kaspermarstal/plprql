#!/bin/sh
set -eu

if ! id -u postgres >/dev/null 2>&1; then
    echo "[!] User 'postgres' does not exist. Have the official Postgres packages been installed yet?" >&2
    exit 1
fi
