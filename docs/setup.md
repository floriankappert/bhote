# Setup wizard

On the first start on a machine, bhote opens a short wizard, once and in one panel only (herdr may start several panels at
a time). Later it is under **settings › Setup wizard**, or `bhote setup` in any terminal. Every step explains itself and
asks first: `⏎` takes the `[default]`, `s` skips a step, `q` (or `Ctrl-C`) ends the setup. On the welcome screen that
offers it, any key starts it, `Esc` postpones it to the next start and `q` quits. `bhote setup --json` only reports what is
in place and changes nothing (it asks a machine over ssh only when herdr reaches it).

| Step | |
|---|---|
| 1 Checks | jq, herdr and its server, `bhote` on the PATH, gzip/base64/perl, ssh into this machine, Tailscale (also the macOS app), the herdr plugin (can link it, also from a Homebrew or Arch package), the Claude Code `SessionStart` hook and the bhote skill (can add them). |
| 2 This machine | Its name in the agent lists (`HOST_LABEL`); other machines must know it by the same name. A rename moves the agent links of the topics along; rename it in herdr on the other machines, too. |
| 3 Other machines | The machines saved in herdr, with a reachability test; finds new ones (your own Tailscale devices, Bonjour/Avahi in the local network, `~/.ssh/config`) and adds them with `herdr machine add`. The scan does not remember any host key; only the machine you pick is. |
| 4 The other way round | Makes the other machines know this one, so that they see its agents and topics travel both ways (below). |
| 5 Topics | Only here, or in step with a machine through herdr (recommended) or ssh; sets up the other side (through herdr only when it knows this machine, step 4), then merges once. |
| 6 Remote agents | Show the agents of the other machines. |
| 7 Agents and notifications | May an agent close its topic, herdr notifications, the waiting reminder, tests = busy. |
| 8 The panel | Autostart with herdr, the animation. |
| 9 Done | A summary. |

## The other way round, in detail

For each saved machine that does not know this one yet, and only when you confirm:

1. **ssh into this machine** must be open. When it is closed, the wizard offers to switch it on (macOS:
   `sudo launchctl load -w /System/Library/LaunchDaemons/ssh.plist`; Linux: `sudo systemctl enable --now sshd`, `ssh` on
   Debian/Ubuntu; sudo asks for the password itself), to open the settings (macOS: Sharing › Remote Login), or to skip it.
2. **The address** of this machine as the other one sees it: its Tailscale name, its local network name or its IP address
   (pick one or type it).
3. **The other machine's ssh key** is read (or, if you agree, created) and shown with its fingerprint. Only when you say yes
   is it added to `~/.ssh/authorized_keys` here (once: a key that is there already is not added again). If you say no, the
   wizard explains what you lose: that machine shows no agents of this one, and topics travel one way only.
4. A test login from there, then `herdr machine add` there.

**Passwords:** the wizard never asks for one. When ssh needs a password (for example before the keys are in place), ssh asks
for it itself, in the terminal; bhote never sees it and stores nothing.
