---
name: bhote-install
description: Install and set up bhote (the terminal workspace with its own multiplexer, the topics and agents panels; herdr is not needed) completely on this machine, through its CLI, without the interactive wizard: the program and the daemon (bhoted), jq, the Claude Code hook and skills in every Claude profile, the settings, the connection to other machines (kharka over ssh), the monitors, the Omarchy bar widget. Use it when the user asks to install, set up, repair, update or check bhote on a machine ("bhote-install", "install bhote here", "set up bhote on the Mac").
---

# bhote-install: set bhote up completely

You do what the setup wizard does, but through commands, and you check each step. Work in this order; after every step
say in one line what is in place. **Ask the user before**: anything with a secret (tokens are never printed or put on a
command line), opening ssh to another machine, stopping an old herdr-kharka service, enabling Slack/CircleCI, and restarting
panels that may show a wizard. Never restart the Omarchy shell while the session is locked: touching a plugin file
reloads it.

## 1. Look first (changes nothing)

```sh
uname -s; command -v bhote bhoted jq gh git ssh curl claude cargo
bhote version; bhote doctor                 # what is in place: the daemon, the mux server, jq, the keymap, kharka
bhote integration status claude             # the Claude Code hook, per profile
echo "$CLAUDE_CONFIG_DIR"; ls -d ~/.claude* 2>/dev/null     # every Claude Code profile on this machine
```

## 2. The program (herdr is not needed)

- From a checkout, any macOS or Linux: `git clone https://github.com/floriankappert/bhote && cd bhote && ./install.sh`. It links `~/.local/bin/bhote`, builds the daemon
  `bhoted` (needs Rust: https://rustup.rs; or `cargo build --release --manifest-path daemon/Cargo.toml`), installs the Claude Code hook in every profile
  and, on Omarchy, the widget `bhote.bar`. Needs `jq`. `gh` is optional (deployment and test monitor).
- Check: `bhote version`, `bhote doctor` (it says "herdr is not needed").
- Start it: type `bhote` in a plain terminal (it starts the pane server and attaches; `ctrl+b d` detaches; `bhote mux stop|status|restart`). Keys: `~/.config/bhote/keys`
  (`prefix = ctrl+space` for herdr's old prefix). The panels, the chat strip and the sections are settings: `PANEL_LEFT`, `PANEL_RIGHT`, `CHAT_PANEL` (docs/mux.md).
- If herdr is still installed and should stay what `bhote` starts: `bhote config set MUX herdr` (the plugin is linked by install.sh then).

## 3. Omarchy bar widget (Linux/Omarchy only)

`install.sh` links it. Then `omarchy plugin enable bhote.bar --section right` and `omarchy plugin list | grep bhote`.
Reload by `touch ~/.config/omarchy/plugins/bhote.bar/BarWidget.qml`; read errors with `timeout 6 qs log -p /usr/share/omarchy/shell | grep -i bhote`.

## 4. Claude Code: hook and skills, in EVERY profile

For each profile directory `<dir>` (`${CLAUDE_CONFIG_DIR:-~/.claude}` and every other `~/.claude*` the user uses):

- the state hook: `bhote integration install claude` (every profile; it tells the mux exactly what a session does: working, blocked, idle, its session id).
- the hook: `<dir>/settings.json` needs a SessionStart command `command -v bhote >/dev/null 2>&1 && bhote current 2>/dev/null; true`
  (`grep -q 'bhote current' <dir>/settings.json`). `settings.json` may be a symlink into a config repo: write THROUGH it
  (`cat new > file`), never replace it with `mv`.
- the skills: copy or link `integrations/claude/skills/bhote` (and `bhote-install`) to `<dir>/skills/<name>/` (`bhote skills install` does it for every profile).
- the mod (optional, Claude Code 2.1.287+): `bhote mod install` puts `bhote-topic` (the topic as a band above the prompt, `/topic`) into every profile; `bhote mod status` checks it. Tell the user to restart Claude Code or run `/reload-plugins`.

## 5. Settings (all through `bhote config`, nothing by hand)

```sh
bhote config list                         # every setting with its value
bhote config set STORE kharka; bhote config set AGENTS_VIA kharka   # topics and agents of all machines through kharka (see 6)
bhote config set REMOTE_AGENTS on         # the agents of the other machines too
bhote config set KHARKA_LABEL <name>      # this machine's name in kharka (e.g. Mac, Omarchy)
bhote config set DEPLOY_MONITOR on; bhote config set TEST_MONITOR on
bhote config set CLAUDE_CONFIG_DIR ~/.claude2 # the profile with the Slack connector (only if Slack watch is wanted)
```

Machine codes (three capitals, MAC/OMR) are set in the panel's settings › Connections. The CircleCI token: ask the user to enter it
in settings › Connections (or `bhote config set CIRCLECI_TOKEN …` typed by the user); never read it back.

## 6. Other machines (kharka over ssh)

Machines see each other through kharka: one machine runs `kharka serve` (the hub), every machine `kharka daemon --hub <ssh destination of the hub>` (kharka reaches the hub
over ssh: an ssh tunnel, no open port); Tailscale makes the names resolve. With `STORE=kharka` and `AGENTS_VIA=kharka` the native mux runs `herdr-kharka` itself (the sidecar:
it writes this machine's workspaces and agents into kharka and runs the commands other machines send: focus, prompt) and the panels list every machine's workspaces and agents.
A click on a workspace or an agent of another machine opens it here over ssh (`bhote view` there; `REMOTE_SSH` changes the ssh command; the kharka node name is the host).
Install bhote the same way on every machine. Stop an old `herdr-kharka` service on a machine that runs the mux (Linux: `systemctl --user disable --now herdr-kharka`;
macOS: `launchctl bootout gui/$(id -u)/dev.kharka.herdr`); `bhote doctor` says which one runs. ssh must work from each machine to the others without a password
(`ssh <node> true`).

## 7. Project CI (monitors)

`bhote monitor detect` finds each project's CI from its GitHub repository (needs `gh auth login`); `bhote monitor check` runs one round;
`bhote deploys` / `bhote tests` list the entries.

## 8. Start and verify

- Panels: `bhote reload` restarts the side panels of the mux (it never types into a panel). Run it on each machine (`ssh <host> bhote reload`). Never use `herdr pane run`/`send-keys` on a pane that may still run bhote: the text lands in the panel as keys (t, then Enter, starts a hand-over).
- `bhote doctor` shows the daemon, the mux server, the keymap and kharka; `bhote setup --json` jq, claude_hook and every machine reachable.
- `bhote bar`, `bhote list`, `bhote update` (is a newer version out, and the command that updates it).
- From a checkout: `for t in tests/*.sh; do bash $t | grep ^FAIL; done` (no output = fine).

Finish with a short list: what was installed or changed, what the user still has to do (log in with `gh auth login`, enter a token, unlock the screen).
