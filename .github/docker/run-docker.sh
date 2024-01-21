#! /usr/bin/env bash

# Environment variables:
#   PG_MAJOR_VER: The major version of Postgres in which to build/run. E.g. 14, 12, 15
#   DOCKERFILE_ID: The Dockerfile identifier to be built, included in this repo,
#                  e.g. debian:bullseye or amazon:2
#   CARGO_LOCKED_OPTION: Set to '--locked' to use "cargo --locked", or set to
#                        blank '' to use "cargo" without "--locked"

# Examples of running this script in CI (currently Github Actions):
#   ./.github/docker/run-docker.sh 14 debian_bullseye
#   ./.github/docker/run-docker.sh 12 fedora

set -x

PG_MAJOR_VER=$1
DOCKERFILE_ID=$2

echo "Building docker container for Postgres version $PG_MAJOR_VER on $DOCKERFILE_ID"
echo "Cargo lock flag set to: '$CARGO_LOCKED_OPTION'"

docker build \
  --build-arg PG_MAJOR_VER="$PG_MAJOR_VER" \
  -t plprql \
  -f ".github/docker/Dockerfile.$DOCKERFILE_ID" \
  .

echo "Packaging PL/PRQL for Postgres version $PG_MAJOR_VER on $DOCKERFILE_ID"

docker run plprql \
  cargo test \
  --no-default-features \
  --features "pg$PG_MAJOR_VER" \
  "$CARGO_LOCKED_OPTION"
