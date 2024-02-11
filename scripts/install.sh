#!/bin/bash

# Exit immediately if a command exits with a non-zero status or if an unset variable is used.
set -eu

# Function to check if a command exists
command_exists() {
  command -v "$1" >/dev/null 2>&1
}

# Check if cargo is on the path
if ! command_exists cargo; then
  echo "cargo is not installed. Please install it and try again."
  exit 1
fi

# Check if git is on the path
if ! command_exists git; then
  echo "git is not installed. Please install it and try again."
  exit 1
fi

# Check if jq is on the path
if ! command_exists jq; then
  echo "jq is not installed. Please install it and try again."
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

# Fetch the project version for pgrx
PGRX_VERSION=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="pgrx") | .version')
echo "PL/PRQL pgrx version: ${PGRX_VERSION}"

# Check if the cargo binary "pgrx" is installed
if cargo pgrx --version &>/dev/null; then
  PGRX_VERSION_INSTALLED=$(cargo pgrx --version | awk '{print $2}')
  PGRX_VERSION=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="pgrx") | .version')

  if [[ "$PGRX_VERSION_INSTALLED" != "$PGRX_VERSION" ]]; then
    echo "Installed version of pgrx ($PGRX_VERSION_INSTALLED) does not match the project's version ($PGRX_VERSION)."
    exit 1
  fi
else
  PGRX_VERSION=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="pgrx") | .version')
  cargo install --locked --version="$PGRX_VERSION" cargo-pgrx
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

# Install pgrx with specified options
cd plprql
cargo pgrx install --no-default-features --release --sudo --pg-config="${PG_CONFIG_PATH}"
echo "PL/PRQL installed successfully."
