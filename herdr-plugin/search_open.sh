#!/bin/sh
# Action "search": opens the plugin pane "search" (a popup titled "bhote search"). Bound to the find key (bhote settings).
exec "${HERDR_BIN_PATH:-herdr}" plugin pane open --plugin "${HERDR_PLUGIN_ID:-bhote.panel}" --entrypoint search >/dev/null 2>&1
