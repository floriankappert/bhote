# Setup wizard

On the first start on a machine, bhote opens a short wizard, once and in one panel only (herdr may start several panels at
a time). Later it is under **settings › Setup wizard**, or `bhote setup` in any terminal. Every step explains itself and
asks first: `⏎` takes the `[default]`, `s` skips a step, `q` ends the setup. `bhote setup --json` only reports what is in
place and changes nothing.

| Step | |
|---|---|
| 1 Checks | jq, herdr and its server, `bhote` on the PATH, gzip/base64/perl, the herdr plugin (can link it), the Claude Code `SessionStart` hook (can add it). |
| 2 This machine | Its name in the agent lists (`HOST_LABEL`); other machines must know it by the same name. |
| 3 Other machines | The machines saved in herdr, with a reachability test; adds new ones with `herdr machine add`. |
| 4 The other way round | Makes the other machines know this one, so that they see its agents and topics travel both ways (below). |
| 5 Topics | Only here, or in step with a machine through herdr (recommended) or ssh; sets up the other side, then merges once. |
| 6 Remote agents | Show the agents of the other machines. |
| 7 Agents and notifications | May an agent close its topic, herdr notifications, the waiting reminder, tests = busy. |
| 8 The panel | Autostart with herdr, the animation. |
| 9 Done | A summary. |

## The other way round, in detail

For each saved machine that does not know this one yet, and only when you confirm:

1. **ssh into this machine** must be open (macOS: System Settings › General › Sharing › Remote Login; Linux:
   `sudo systemctl enable --now sshd`). The wizard checks and explains; it does not switch it on (that needs an admin).
2. **The address** of this machine as the other one sees it, e.g. `flo@MacBook-Pro.local` (suggested, editable).
3. **The other machine's ssh key** is read (or, if you agree, created) and shown with its fingerprint. Only when you say yes
   is it added to `~/.ssh/authorized_keys` here. If you say no, the wizard explains what you lose: that machine shows no
   agents of this one, and topics travel one way only.
4. A test login from there, then `herdr machine add` there.

**Passwords:** the wizard never asks for one. When ssh needs a password (for example before the keys are in place), ssh asks
for it itself, in the terminal; bhote never sees it and stores nothing.
