#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROFILE_DIR=debug

if [ "${1:-}" = "--release" ]; then
  PROFILE_DIR=release
fi

BIN="$ROOT_DIR/target/$PROFILE_DIR/wx_station"

if [ ! -x "$BIN" ]; then
  cd "$ROOT_DIR"
  if [ "$PROFILE_DIR" = "release" ]; then
    cargo build --release
  else
    cargo build
  fi
fi

exec "$BIN"
