---
name: bhote-install
description: Install and set up bhote (the topics and agents side panel for herdr) completely on this machine, through its CLI and the herdr API, without the interactive wizard: the program, jq/herdr, the herdr plugin, the Omarchy bar widget, the Claude Code hook and skills in every Claude profile, the settings, the connection to other machines, the monitors. Use it when the user asks to install, set up, repair, update or check bhote on a machine ("bhote-install", "install bhote here", "set up bhote on the Mac").
---

# bhote-install: set bhote up completely

You do what the setup wizard does, but through commands, and you check each step. Work in this order; after every step
say in one line what is in place. **Ask the user before**: anything with a secret (tokens are never printed or put on a
command line), opening ssh to another machine, changing herdr's `config.toml`, enabling Slack/CircleCI, and restarting
panels that may show a wizard. Never restart the Omarchy shell while the session is locked: touching a plugin file
reloads it.

## 1. Look first (changes nothing)

```sh
uname -s; command -v bhote herdr jq gh git ssh curl claude
bhote version; bhote setup --json          # what is in place: jq, herdr, plugin, claude_hook, machines
herdr plugin list; herdr machine list
echo "$CLAUDE_CONFIG_DIR"; ls -d ~/.claude* 2>/dev/null     # every Claude Code profile on this machine
```

## 2. The program

- macOS or Linux with Homebrew: `brew install floriankappert/bhote/bhote`, then `herdr plugin link "$(brew --prefix)/share/bhote/herdr-plugin"`.
- Arch/Omarchy: `git clone https://github.com/floriankappert/bhote && cd bhote/packaging/arch && makepkg -si`.
- Any other system, or to work from a checkout: `git clone https://github.com/floriankappert/bhote && cd bhote && ./install.sh`
  (links `~/.local/bin/bhote`, the herdr plugin `bhote.panel` and, on Omarchy, the widget `bhote.bar`).
- Needs `jq` and `herdr` (`brew install jq` / `pacman -S jq`; herdr from herdr.dev). `gh` is optional (deployment and test monitor).

Check: `bhote version`, `herdr plugin list | grep bhote.panel`.

## 3. Omarchy bar widget (Linux/Omarchy only)

`install.sh` links it. Then `omarchy plugin enable bhote.bar --section right` and `omarchy plugin list | grep bhote`.
Reload by `touch ~/.config/omarchy/plugins/bhote.bar/BarWidget.qml`; read errors with `timeout 6 qs log -p /usr/share/omarchy/shell | grep -i bhote`.

## 4. Claude Code: hook and skills, in EVERY profile

For each profile directory `<dir>` (`${CLAUDE_CONFIG_DIR:-~/.claude}` and every other `~/.claude*` the user uses):

- the hook: `<dir>/settings.json` needs a SessionStart command `command -v bhote >/dev/null 2>&1 && bhote current 2>/dev/null; true`
  (`grep -q 'bhote current' <dir>/settings.json`). `settings.json` may be a symlink into a config repo: write THROUGH it
  (`cat new > file`), never replace it with `mv`.
- the skills: copy or link `integrations/claude/skills/bhote` (and `bhote-install`) to `<dir>/skills/<name>/`.

## 5. Settings (all through `bhote config`, nothing by hand)

```sh
bhote config list                         # every setting with its value
bhote config set STORE remote; bhote config set STORE_MACHINE <label>; bhote config set SYNC_VIA herdr
bhote config set REMOTE_AGENTS on         # the agents of the saved machines too
bhote config set HERDR_KEYS_MACHINE <label>   # the machine whose herdr you TYPE into (a Mac attached to a Linux session): its keys are shown
bhote config set DEPLOY_MONITOR on; bhote config set TEST_MONITOR on
bhote config set CLAUDE_CONFIG_DIR ~/.claude2 # the profile with the Slack connector (only if Slack watch is wanted)
```

Machine codes (three capitals, MAC/OMR) are set in the panel's settings › Connections. The CircleCI token: ask the user to enter it
in settings › Connections (or `bhote config set CIRCLECI_TOKEN …` typed by the user); never read it back.

## 6. Other machines

`herdr machine list` shows saved machines; add one with `herdr machine add` (the user authenticates) and check
`herdr machine status`. Both machines must know each other (ssh both ways; Tailscale makes names resolve). bhote itself reports it
in `bhote setup --json` (`reachable`, `knows_this_machine`). Install bhote on the other machine the same way, with the SAME machine name.

## 7. Project CI (monitors)

`bhote monitor detect` finds each project's CI from its GitHub repository (needs `gh auth login`); `bhote monitor check` runs one round;
`bhote deploys` / `bhote tests` list the entries.

## 8. Start and verify

- Panels: `bhote reload` restarts every bhote panel of this machine safely (it ends the old panel and only then starts the new one; it never types into a panel that still runs). Run it on each machine (`ssh <host> bhote reload`). Never use `herdr pane run`/`send-keys` on a pane that may still run bhote: the text lands in the panel as keys (t, then Enter, starts a hand-over). Ask first if a panel might be inside the wizard.
- `bhote setup --json` shows jq, herdr, plugin, claude_hook true and every machine reachable.
- `bhote bar`, `bhote list`, `bhote update` (is a newer version out, and the command that updates it).
- From a checkout: `for t in tests/*.sh; do bash $t | grep ^FAIL; done` (no output = fine).

Finish with a short list: what was installed or changed, what the user still has to do (log in with `gh auth login`, enter a token, unlock the screen).
