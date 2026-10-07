# herdr plugin

`herdr-plugin/` is a herdr plugin (`bhote.panel`) that puts the panel next to your agents. `./install.sh` links it
(`herdr plugin link <repo>/herdr-plugin`).

| Hook | When | Does |
|---|---|---|
| `[[startup]]` | herdr starts (and on server handoff) | In every tab: split the agent pane (or the first pane) to the right, about 44 columns, and run `bhote` in the new pane. |
| `[[events]] worktree.created` | a worktree was created | the same, for tabs that have no panel yet |
| action `Bhote: open panel` | by hand | the same for the focused tab only, also when the autostart is off |

Rules the plugin follows:

- A tab that already shows a pane titled `Bhote` is left alone.
- It labels the panes it creates `bhote`. A pane with that label that does not run bhote any more (restored after a restart)
  gets `bhote` started in it. A pane is never typed into because of its title alone, and never when it runs an agent.
- It splits only when the agent keeps at least 64 columns (`BHOTE_AGENT_MIN_WIDTH`); the panel width is `BHOTE_PANEL_WIDTH`
  (44).
- `AUTOSTART=off` in the settings switches the startup and event hooks off.
- `BHOTE_DRY=1 sh open_panel.sh startup` prints what it would do and changes nothing.

It needs `jq`, and finds `bhote` on `PATH` or in `~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`.
