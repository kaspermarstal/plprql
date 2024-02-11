#!/bin/bash

# Exit immediately if a command exits with a non-zero status or if an unset variable is used.
set -exu

# Function to check if a command exists
command_exists() {
  command -v "$1" >/dev/null 2>&1
}

# Check if cargo is on the path
if ! command_exists cargo; then
  echo "cargo is not installed. Please install it and try again."
  echo "See https://www.rust-lang.org/tools/install"
  exit 1
fi

# Check if cargo pgrx is installed
if ! command_exists cargo; then
  echo "cargo is not installed. Please install it and try again."
  exit 1
fi

# Check if git is on the path
if ! command_exists git; then
  echo "git is not installed. Please install it and try again."
  echo "See https://git-scm.com/downloads"
  exit 1
fi

# Check if jq is on the path
if ! command_exists jq; then
  echo "jq is not installed. Please install it and try again."
  echo "See https://jqlang.github.io/jq/download/"
  exit 1
fi

# Parse command-line arguments for pg_config path and revision
PG_CONFIG_PATH=""
REVISION=""
while [[ "$#" -gt 0 ]]; do
  case $1 in
    --pg-version) PG_CONFIG_PATH="$2"; shift ;;
    --revision) REVISION="$2"; shift ;;
    *) echo "Unknown parameter: $1"; exit 1 ;;
  esac
  shift
done

# Create a temporary directory for compilation
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT

# Clone the project
git clone https://github.com/kaspermarstal/plprql "${TEMP_DIR}"

# Change to the temporary directory
cd "${TEMP_DIR}"

# If a revision is specified, checkout that revision
if [[ -n "$REVISION" ]]; then
  git checkout "$REVISION"
fi

# Fetch PL/PRQL version for pgrx
PGRX_VERSION=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="pgrx") | .version')

# Check if cargo pgrx is installed
if ! command -v cargo pgrx >/dev/null 2>&1; then
  echo "cargo pgrx is not installed. Please install version $PGRX_VERSION and try again."
  echo "See https://github.com/pgcentralfoundation/pgrx"
  exit 1
fi

# Check if the required version of pgrx is installed
PGRX_VERSION_INSTALLED=$(cargo pgrx --version | awk '{print $2}')
if [[ "$PGRX_VERSION_INSTALLED" != "$PGRX_VERSION" ]]; then
  echo "Installed version of pgrx ($PGRX_VERSION_INSTALLED) does not match the project's version ($PGRX_VERSION)."
  echo "Please install version $PGRX_VERSION and try again."
  exit 1
fi

# Check if pg_config path is provided and exists
if [[ -n "$PG_CONFIG_PATH" ]] && ! command -v "$PG_CONFIG_PATH" &>/dev/null; then
  echo "The specified pg_config path does not exist."
  exit 1
fi

# Fallback to default pg_config if not provided or doesn't exist
if [[ -z "$PG_CONFIG_PATH" ]] && ! command -v pg_config &>/dev/null; then
  echo "pg_config is not installed or not found in PATH."
  exit 1
fi

# Use the provided pg_config path or fallback to the default
PG_CONFIG_PATH="${PG_CONFIG_PATH:-$(which pg_config)}"

# Install PL/PRQL
cd plprql
cargo pgrx install --no-default-features --release --sudo --pg-config="${PG_CONFIG_PATH}"
echo "PL/PRQL installed successfully."
