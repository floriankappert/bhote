# The herdr Mac app add-on

Since 0.9.3 bhote is a **native add-on for the herdr Mac app**: a mod (`herdr-app/mods/bhote`) with a layout of its own (`herdr-app/layouts/bhote.json`). The app draws its panels, so it is not a terminal UI;
the terminal UI of earlier versions is [deprecated](panel.md). The add-on asks the same CLI and reads the same files as everything else: the topics, the lists the collector writes, kharka.

## Install and update

```sh
bhote app install        # copies the mod and the layout into ~/.config/herdr-app (HERDR_APP_HOME picks another folder), then checks the mod
bhote app check          # loads the mod once and reports what is wrong (herdr-app check)
herdr-app do layout.show --id bhote
```

`brew upgrade bhote && bhote app install` updates it; the app reloads a changed mod by itself. The files come from a checkout or from the package (`share/bhote/herdr-app`).

## What it shows

| | |
|---|---|
| **Overview** | Tiles for topics, pull requests, deployments, tests, servers and runbooks; a tab for each. |
| **Topics** | Statuses, the topic's page, the stash. *Start on agent* hands a topic over as a message and waits (spinner) until the agent takes it; then the agent is fixed. |
| **Agents** (left panel) | Cards under their machine (alias: `MACHINE_LABELS`, e.g. `omarchy-flo=Omarchy`), a name from Middle-earth per agent, state by colour, drag and drop for the order, the ear. |
| **Commit inspector** | The last ten commits of the focused agent's branch since it left the base branch; a click opens the message above it. |
| **Headless agents** | Scheduled Claude jobs (systemd timers, on Linux machines), the machine of each, run now, pause, stop; the log as a small conversation; a form to start one more run. |
| **Servers** | The IMS systems per machine, your dev servers (`~/.config/bhote/servers.json`), the SSH tunnel (`IMS_TUNNELS`). |
| **Pull requests, tests, deployments** | PR cards with their checks; the Tests tab with local runs, CI jobs and test processes found on every machine; the workflows of a CircleCI pipeline as one card. |
| **Settings** | The gear row at the foot: the settings that concern the app, the runbooks of your projects, the servers. |

What comes in (a new card or row) grows in, fades in and lights up for a moment; nothing animates when a list is first shown.

## Agents and messages

Every agent has a name from Middle-earth (60 short names, then `Frodo2` …): the same on every machine, from the rank of its record in `bhote.agentnums`. It is a ref for the CLI (`bhote jump frodo`).
Agents message each other with `bhote send <agent> <text>`: the message is kept in kharka (`msg/<node>/<name>/<id>`) and the app hands it to the agent, as a one-line prompt that says where to read it,
when the **ear** on the agent's card is on and the agent is free; with the ear off, or while it works, it waits (the card counts them). The sender is the agent the command is called from; a message from you says `the user`.
The ear is the record `listen/<node>/<name>` in kharka (`{"listening": false}`); without one, `AGENT_LISTEN` counts.

## Dev servers

A service is a name, a machine, a port and a start command run in your dev shell (`zsh -ic`), kept in `~/.config/bhote/servers.json` ([configuration](configuration.md#services-herdr-app)). It runs while its port answers.
On Linux it starts as a systemd user unit, on macOS as a detached process group. A start that does not work (its process ends without the port answering, or no answer in two minutes) turns the card red with the last lines of its log
and becomes a topic in review (`<name> did not start on <machine>`), in the project of the same name.

## Where things are

| | |
|---|---|
| `~/.config/herdr-app/mods/bhote/` | the mod (`main.js`, its icons, `servers.js`, `monitors.py`) |
| `~/.config/herdr-app/layouts/bhote.json` | the layout |
| `~/.config/bhote/config`, `servers.json` | the settings, the dev servers |
| `~/.cache/bhote/svc/`, `~/.cache/bhote/headless/` | logs of dev servers and of runs you started |

Removing it: `rm -r ~/.config/herdr-app/mods/bhote ~/.config/herdr-app/layouts/bhote.json`. Your topics and settings stay.
