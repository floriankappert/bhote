# Configuration

Settings live in one file, `~/.config/bhote/config` (`$XDG_CONFIG_HOME/bhote/config`, or `BHOTE_CONFIG`). It holds plain
`KEY=value` lines and is **never sourced**: bhote reads it line by line, so a value can never run as code. The panel's
settings screen (`,`) writes the common keys; the rest are for this file. Unknown keys are kept as they are.

Nothing that reaches another machine is on by default: bhote opens no ssh connection until `STORE=remote` or
`REMOTE_AGENTS=on` says so.

| Key | Default | Settings screen | |
|---|---|---|---|
| `STORE` | `local` | Data location | `remote` keeps a replica of the topics on `STORE_MACHINE` ([data and sync](data-and-sync.md)). |
| `STORE_MACHINE` | | Data location | The label of a saved herdr machine (`herdr machine list`). |
| `REMOTE_AGENTS` | `off` | Load remote agents over SSH | List the agents of the saved herdr machines, too. |
| `MACHINES_OFF` | | one row per machine | Comma-separated labels that stay out even with `REMOTE_AGENTS=on`. |
| `LOCAL_EVERY` | `3` | Local interval (s) | Seconds between two queries of this machine's agents. |
| `REMOTE_EVERY` | `10` | Remote interval (s) | Seconds between two queries of the other machines (and the regular sync). |
| `TESTS_BUSY` | `off` | Running tests = agent busy | An agent whose herdr workspace runs tests (its `tests` token starts with `◌`) counts as busy. |
| `ANIMATION` | `on` | Animation | The dog on the welcome screen blinks and pants. |
| `SPLASH_AUTOCLOSE` | `on` | Close welcome screen after 15 s | |
| `SPLASH_SECONDS` | `15` | | Seconds until the welcome screen closes itself. |
| `AUTOSTART` | `on` | Auto-start with herdr | The [herdr plugin](herdr-plugin.md) opens the panel when herdr starts. |
| `AGENT_ROWS` | automatic | drag the `═══` divider | Height of the agent area in rows. |
| `DONE_MAX` | `7` | | Done topics shown in the list before `+n more`. |
| `HOST_LABEL` | `Mac` on macOS, else the host name | | How this machine is called in agent lists. Should match the label other machines use for it in `herdr machine list`. |
| `NAME_COLORS` | | | Colour agents by part of their name: `api=teal,web=mauve` (case-insensitive, first match wins). Colours: `red teal peach mauve blue green yellow`. |

Numbers that are not plain digits fall back to their default. Values never contain control characters (they are removed when
written).
