# herdr plugin

`herdr-plugin/` is a herdr plugin (`bhote.panel`) that puts the panel next to your agents. `./install.sh` links it
(`herdr plugin link <repo>/herdr-plugin`).

| Hook | When | Does |
|---|---|---|
| `[[startup]]` | herdr starts (and on server handoff) | In every tab: split the agent pane (or the first pane) to the right, about 44 columns, and run `bhote` in the new pane. |
| `[[events]] worktree.created` | a worktree was created | the same, for tabs that have no panel yet |
| `[[events]] pane.agent_status_changed` | an agent's status changed | `bhote event`: a finished agent moves its topic to review (with a notification); all panels refresh at once |
| action `changed` | another machine's `bhote event` | That machine's agents changed: bhote asks it again at once (`PUSH_STATES`). |
| action `Bhote: open panel` | by hand | the same for the focused tab only, also when the autostart is off |
| action `Bhote: jump to the panel and back` (`bhote.panel.focus`) | a key you bind | Focuses the bhote panel of the tab; from the panel, back to the pane you came from. A tab without a panel gets one first. |

Bind the jump to a key in herdr's `config.toml` (the setup wizard offers it); the panel shows the key bottom right:

```toml
[[keys.command]]
key = "prefix+t"
type = "plugin_action"
command = "bhote.panel.focus"
description = "Bhote panel"
```

Rules the plugin follows:

- A tab that already shows a pane titled `Bhote` is left alone.
- It labels the panes it creates `bhote`. A pane with that label that does not run bhote any more (restored after a restart)
  gets `bhote` started in it. A pane is never typed into because of its title alone, and never when it runs an agent.
- It splits only when the agent keeps at least 64 columns (`BHOTE_AGENT_MIN_WIDTH`); the panel width is `BHOTE_PANEL_WIDTH`
  (44).
- `AUTOSTART=off` in the settings switches the startup and event hooks off.
- `BHOTE_DRY=1 sh open_panel.sh startup` prints what it would do and changes nothing.

It needs `jq`, and finds `bhote` on `PATH` or in `~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`.
