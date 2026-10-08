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
| `PUSH_STATES` | `on` | Push agent states at once | With `REMOTE_AGENTS` on: when an agent here changes its state, the other machines are told at once through herdr (the plugin action `changed`), so their panels show it within about a second instead of after `REMOTE_EVERY`. |
| `REMOTE_AGENTS` | `off` | Remote agents (via herdr) | List the agents of the saved herdr machines, too (herdr asks them over its ssh connection). |
| `MACHINES_OFF` | | one row per machine | Comma-separated labels that stay out even with `REMOTE_AGENTS=on`. |
| `PANEL_MIN_WIDTH` | `36` | Panel width min | The narrowest the panel's herdr pane may get; after a resize a narrower one is widened again (the agent next to it keeps at least 40 columns). |
| `PANEL_MAX_WIDTH` | `60` | Panel width max | The widest it may get; a wider one is narrowed again. Also the widest the panel draws. |
| `LOCAL_EVERY` | `3` | Local interval (s) | Seconds between two queries of this machine's agents. |
| `REMOTE_EVERY` | `5` | Remote interval (s) | Seconds between two queries of the other machines (and the regular sync). |
| `TESTS_BUSY` | `off` | Running tests = agent busy | An agent whose herdr workspace runs tests (its `tests` token starts with `◌`) counts as busy. |
| `AUTO_ASSIGN` | `off` | Auto-assign now topics | A running (`now`) topic without an agent goes to the next free agent by itself, 20 s after its last change (time to add a description); the agent is checked live first and you get a notification. It runs once per machine: with two machines sharing topics, switch it on on one of them. |
| `AUTO_ASSIGN_MACHINE` | `any` | Prefer agents on | Whose free agents come first: `any` (this machine first) or a machine label; when none is free there, the others. |
| `AGENT_CAN_CLOSE` | `on` | Agents may close topics | An agent may set its topic to done itself; `off`: only to review, you close it. |
| `NOTIFY` | `on` | herdr notifications | herdr notifications when a topic is ready for review or done, and when a waiting topic waits too long. |
| `GITHUB_BRANCHES` | `on` | GitHub: agent branches | The git branch the agent of a topic works on, as a line under the topic (and inline in the search). Each machine reads it for its own agents from their working folder (`.git`, worktrees too) and writes it into the topic when it changes. |
| `SLACK_WATCH` | `off` | Watch Slack pins (Claude) | This machine watches the Slack channels pinned to waiting topics (`bhote slack`) through Claude Code and its Slack connector, and sets a topic to review when someone else writes. One machine is enough. |
| `SLACK_EVERY` | `120` | Slack interval (s) | Seconds between two Slack checks; each is one short Claude call (Haiku), and only while a waiting topic (or one Slack put under review) has a pin. |
| `DEPLOY_MONITOR` | `off` | Deployment monitor | The last deployments of the projects above the agents ([monitors](monitors.md)). |
| `DEPLOY_EVERY` | `60` | Deploy interval (s) | Seconds between two deployment checks. |
| `DEPLOY_GITHUB` · `DEPLOY_CIRCLECI` | `on` | GitHub Actions · CircleCI | The deployment monitor's modules. |
| `TEST_MONITOR` | `off` | Test monitor | The last test runs above the agents: local runs, status files, CI test jobs. |
| `TEST_EVERY` | `60` | Test interval (s) | Seconds between two test checks. |
| `TEST_LOCAL` · `TEST_GITHUB` · `TEST_CIRCLECI` | `on` | Local runs · GitHub Actions · CircleCI | The test monitor's modules. |
| `TEST_STATUS_DIR` | `~/.cache/ims-test-status` | Status files | Folder with live test status files (one JSON per suite, as IMS writes them). |
| `UPDATE_CHECK` | `on` | Check for updates | The collector asks GitHub for the newest `vX.Y.Z` tag every 6 hours. A newer version shows in Settings and in the head of the panel with the command that updates it (`brew update && brew upgrade bhote`, `git -C <checkout> pull`, …). `bhote update` asks right now. |
| `MONITOR_ROWS` | `7` | Entries shown | Entries each monitor shows. |
| `CIRCLECI_TOKEN` | – | CircleCI token | CircleCI personal API token (a secret: `bhote config` prints `(set)`); else `$CIRCLECI_TOKEN` or `~/.config/zsh/secrets.zsh`. |
| `WAIT_REMIND` | `24` | Remind waiting after (h) | Hours after which a waiting topic is reminded once (`0` = never). |
| `THEME` | `terminal` | Colours | `terminal`: the terminal's own 16 colours, so bhote follows the terminal theme (and Omarchy's system theme) at once. `catppuccin`: exact truecolor values. |
| `COLOR_<ROLE>` | | | Overrides one role with `#rrggbb` in either theme. Roles: `TEXT`, `DIM`, `FRAME`, `SELECTION` (background), `ACCENT`, `NOW` (also busy agents), `WAITING`, `REVIEW`, `LATER`, `DONE` (also free agents), `ERROR`. Example: `bhote config set COLOR_NOW '#fab387'`. |
| `ANIMATION` | `on` | Animation | The dog on the welcome screen blinks and pants; busy agents show Claude's dancing star. |
| `SPLASH_AUTOCLOSE` | `on` | Close welcome screen after 15 s | |
| `SPLASH_SECONDS` | `15` | | Seconds until the welcome screen closes itself. |
| `AUTOSTART` | `on` | Auto-start with herdr | The [herdr plugin](herdr-plugin.md) opens the panel when herdr starts. |
| `AGENT_ROWS` | automatic | drag the `═══` divider | Height of the agent area in rows. |
| `TOPICS_SCOPE` | `all` | Topics shown | `project`: the panel lists the topics of its project (the project of the agent in its herdr tab) and those without one; `all`: every topic. |
| `DONE_MAX` | `7` | | The last done topics, shown in their own area above the agents (newest first; `0`: none). |
| `HOST_LABEL` | `Mac` on macOS, else the host name | | How this machine is called in agent lists. Should match the label other machines use for it in `herdr machine list`. |
| `JUMP_KEY` | | Jump key (herdr) | The herdr key that jumps between the agent and the bhote panel of the tab (e.g. `prefix+t`; `off` removes it). bhote writes it into herdr's `config.toml` (a block between `# >>> bhote` and `# <<< bhote`) and reloads herdr; a key herdr refuses changes nothing. |
| `FIND_KEY` | | Find key (herdr) | The herdr key that opens `bhote find` as a popup (e.g. `ctrl+alt+t`; on macOS a terminal can send it for ⌘T). Written the same way. |
| `SETUP_DONE` | | | The date the [setup wizard](setup.md) ran on this machine. Empty: it opens at the next start (`bhote config unset SETUP_DONE` brings it back). |
| `PROJECT_COLORS` | `marketing=pink,ims=teal,bilendo=yellow,bhote=green,micro=mauve` | | Colour of the project captions in the agent list: part-of-name=colour, first match wins (`red teal peach mauve blue green yellow pink orange grey`); other projects are grey. A working agent is orange as a whole, one that needs you red, an idle one grey. |
| `NAME_COLORS` | | | Colour agents by part of their name: `api=teal,web=mauve` (case-insensitive, first match wins). Colours: `red teal peach mauve blue green yellow`. |

Numbers that are not plain digits fall back to their default. Values never contain control characters (they are removed when
written).
