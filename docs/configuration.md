# Configuration

Settings live in one file, `~/.config/bhote/config` (`$XDG_CONFIG_HOME/bhote/config`, or `BHOTE_CONFIG`). It holds plain
`KEY=value` lines and is **never sourced**: bhote reads it line by line, so a value can never run as code. The panel's
settings screen (`,`) and `bhote config set` write it; both check every value against its type. Unknown keys are kept as they are.

Nothing that reaches another machine is on by default: bhote opens no ssh connection until `STORE=remote` or
`REMOTE_AGENTS=on` says so.

| Key | Default | Settings screen | |
|---|---|---|---|
| `STORE` | `local` | Data location | `remote` keeps a replica of the topics on `STORE_MACHINE` ([data and sync](data-and-sync.md)). |
| `STORE_MACHINE` | | Data location | The label of a saved herdr machine (`herdr machine list`). |
| `SYNC_VIA` | `herdr` | Sync through | How topics travel to `STORE_MACHINE`: `herdr` (through herdr's own connection; each side pulls from the other) or `ssh` (bhote's own ssh; push and pull). See [data and sync](data-and-sync.md). |
| `REMOTE_AGENTS` | `off` | Remote agents (via herdr) | List the agents of the saved herdr machines, too (herdr asks them over its ssh connection). |
| `MACHINES_OFF` | | one row per machine | Comma-separated labels that stay out even with `REMOTE_AGENTS=on`. |
| `LOCAL_EVERY` | `3` | Local interval (s) | Seconds between two queries of this machine's agents. |
| `REMOTE_EVERY` | `10` | Remote interval (s) | Seconds between two queries of the other machines (and the regular sync). |
| `TESTS_BUSY` | `off` | Running tests = agent busy | An agent whose herdr workspace runs tests (its `tests` token starts with `◌`) counts as busy. |
| `AGENT_CAN_CLOSE` | `on` | Agents may close topics | An agent may set its topic to done itself; `off`: only to review, you close it. |
| `NOTIFY` | `on` | herdr notifications | herdr notifications when a topic is ready for review or done, and when a waiting topic waits too long. |
| `WAIT_REMIND` | `24` | Remind waiting after (h) | Hours after which a waiting topic is reminded once (`0` = never). |
| `THEME` | `terminal` | Colours | `terminal`: the terminal's own 16 colours, so bhote follows the terminal theme (and Omarchy's system theme) at once. `catppuccin`: exact truecolor values. |
| `COLOR_<ROLE>` | | | Overrides one role with `#rrggbb` in either theme. Roles: `TEXT`, `DIM`, `FRAME`, `SELECTION` (background), `ACCENT`, `NOW` (also busy agents), `WAITING`, `REVIEW`, `LATER`, `DONE` (also free agents), `ERROR`. Example: `bhote config set COLOR_NOW '#fab387'`. |
| `ANIMATION` | `on` | Animation | The dog on the welcome screen blinks and pants. |
| `SPLASH_AUTOCLOSE` | `on` | Close welcome screen after 15 s | |
| `SPLASH_SECONDS` | `15` | | Seconds until the welcome screen closes itself. |
| `AUTOSTART` | `on` | Auto-start with herdr | The [herdr plugin](herdr-plugin.md) opens the panel when herdr starts. |
| `AGENT_ROWS` | automatic | drag the `═══` divider | Height of the agent area in rows. |
| `DONE_MAX` | `7` | | Done topics shown in the list before `+n more`. |
| `HOST_LABEL` | `Mac` on macOS, else the host name | | How this machine is called in agent lists. Should match the label other machines use for it in `herdr machine list`. |
| `JUMP_KEY` | | Jump key (herdr) | The herdr key that jumps between the agent and the bhote panel of the tab (e.g. `prefix+t`; `off` removes it). bhote writes it into herdr's `config.toml` (a block between `# >>> bhote` and `# <<< bhote`) and reloads herdr; a key herdr refuses changes nothing. |
| `FIND_KEY` | | Find key (herdr) | The herdr key that opens `bhote find` as a popup (e.g. `ctrl+alt+t`; on macOS a terminal can send it for ⌘T). Written the same way. |
| `SETUP_DONE` | | | The date the [setup wizard](setup.md) ran on this machine. Empty: it opens at the next start (`bhote config unset SETUP_DONE` brings it back). |
| `NAME_COLORS` | | | Colour agents by part of their name: `api=teal,web=mauve` (case-insensitive, first match wins). Colours: `red teal peach mauve blue green yellow`. |

Numbers that are not plain digits fall back to their default. Values never contain control characters (they are removed when
written).
