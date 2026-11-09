#!/bin/sh
set -eu
exec python3 "$(dirname "$0")/prepare_release.py" "$@"
