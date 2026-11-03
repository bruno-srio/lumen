#!/bin/sh
# blueprint-compiler resolves input filenames against the working directory.
set -eu
COMPILER="$1"
OUTDIR="$2"
WIDGETS="$3"
cd "$WIDGETS"
"$COMPILER" batch-compile "$OUTDIR" . \
  window.blp \
  device_card.blp \
  color_popover.blp \
  rename_dialog.blp \
  device_details.blp \
  group_card.blp \
  group_dialog.blp
