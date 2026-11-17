# Secrets and environments

API keys must not travel through a chat: what an agent sees ends up as plaintext in `~/.claude*/projects/**.jsonl`, `history.jsonl`, shell snapshots and in every later
request. `bhote secret` keeps keys in a store, shows them only masked, and gives a command its keys as environment variables without anybody (or any agent) reading them.

**The rule:** a key enters through a password field in the Env tab or a terminal prompt (`bhote secret set NAME`), never through a chat. Agents and Claude see names and
"present", never values. Masked form everywhere: first 6 and last 3 characters and the length, `sk-ant…gAA (108)` (a short value shows less).

## Commands

| Command | |
|---|---|
| `bhote secret set NAME` | Prompts with hidden input (needs a terminal). `--from-file F [--shred]` reads the value from a file (`--shred` overwrites and removes it), `--stdin` from the pipe, `--op op://vault/item/field` stores only a 1Password reference (resolved with `op read` at use time). |
| `bhote secret get NAME` | Masked, with backend and fingerprint. `--reveal` prints the value (refused inside a Claude Code session, see below); `--clip` copies it to the clipboard and clears it after `SECRET_CLIP_SECONDS` (30). |
| `bhote secret ls [--json]` | Names, backend, masked form, fingerprint (first 10 hex of sha256), created/updated, source. Never values. |
| `bhote secret rm NAME` · `rename OLD NEW` | Rename keeps the environments pointing at it. |
| `bhote secret verify [NAME]` | Readable, and the fingerprint matches (`--all`). Exit 1 if not. |
| `bhote secret scan [--json] [--machine M]` | Finds keys in the Claude profiles (`~/.claude*`: transcripts, shell snapshots, session-env, `history.jsonl`, paste-cache), grouped by fingerprint with a name hint, kind, where and last seen. Leaves out paths (`HERDR_*`, `*_PATH`), publishable keys (`pk_test`, `pk_live`, `NEXT_PUBLIC_*`), placeholders, truncated values. `--machine` runs the scanner on another machine through `hc exec --on` (the script is copied to `~/.cache/bhote`, as `monitors.py` is). |
| `bhote secret import --from-scan [--dry-run] [--skip FP,...]` | Stores every found key (`--skip`: fingerprints, or their start, that are no real secrets) under a name (the variable it was assigned to, else `KIND_<fp4>`: `AWS_ACCESS_KEY_ID_ab12`, `GITHUB_TOKEN_…`, `CIRCLECI_TOKEN_…`, `STRIPE_LIVE_KEY_…`) and verifies each by reading it back. |
| `bhote secret import --config [--migrate]` | Stores the secrets of the bhote config (`CIRCLECI_TOKEN`). The config value keeps working; `bhote config set CIRCLECI_TOKEN secret:CIRCLECI_TOKEN` points the config at the store (the daemon understands that after `bhote ship`; run `--migrate` only then). |
| `bhote secret purge-context [--dry-run]` | Replaces each verified value in the Claude profiles with `«secret:NAME#fp»`. See below. |
| `bhote secret machines [--json]` | Read-only matrix machine × secret: present, missing, different (by fingerprint). Nothing is sent between machines; 1Password does that. |
| `bhote env set NAME VAR=SECRET…` · `unset` · `rm` · `ls` | Named sets of variables that point at secrets (`~/.config/bhote/envs.json`: references only, nothing in a repo). |
| `bhote env bind NAME [dir]` · `unbind` · `which` | An environment for a folder (and below it). |
| `bhote env run [NAME] -- cmd` | Runs `cmd` with the variables. Without NAME (or `--auto`) the environment bound to the current folder; none bound: every secret with a name of its own (not a `_xxxx` hash suffix, not `ANTHROPIC_API_KEY`) as the variable of that name, in any folder (`BHOTE_ENV_DEFAULT=off` turns that off). |

`ENV_INJECT on`: a headless agent started from the panel runs through `bhote env run --auto`; a service in `servers.json` with `"env": "NAME"` gets that environment.
Default off.

## The Env tab

Secrets with masked value, backend and presence per machine (`✓` same, `≠` different value, `–` missing), the environments below. **+** opens a form with a password
field. The typed value does not travel in an argument (the panel's `exec` has no stdin, and `ps` shows arguments): the panel creates a folder `~/.cache/bhote/in-…` with
`mkdir -m 700`, writes the value to a file in it, runs `bhote secret set NAME --from-file F --shred`, and removes the folder at once. **Reveal** copies to the clipboard
through `bhote secret get --clip`; the value is never drawn.

## Backends

`SECRET_BACKEND` (default `auto`): macOS **Keychain** (`security`; the value goes in on stdin of `security -i`, never as an argument), **secret-tool** (libsecret; on Linux when a
D-Bus session and an unlocked keyring answer), else an **encrypted file** (`~/.config/bhote/secrets.d/NAME.enc`, `age` if installed, otherwise `openssl enc -aes-256-cbc -pbkdf2`
with a random key in `secrets.key`, both 0600). Each entry remembers its backend. If the Keychain cannot be reached from a process (a sandbox, a background job without
GUI session), bhote says so and does not fall back by itself: use your own terminal, or choose `bhote config set SECRET_BACKEND file` on purpose.

The index `~/.config/bhote/secrets.json` (0600) holds names, backend, masked form, fingerprint, dates. No value.

## Purge: out of the Claude context

`purge-context` rewrites transcripts, snapshots, `session-env`, `history.jsonl` and `paste-cache`. Safeguards: only values that are readable in the store with a matching
fingerprint (others are reported and left alone); files modified in the last 15 minutes are skipped (a session is writing them; a later run gets them), as is the file of the
current session; JSONL is processed line by line and a line that would not be valid JSON afterwards is kept as it was; the rewrite is temp file + rename, keeps mode and
modification time, and is dropped if the file changed meanwhile. Output: masked counts per file and secret. Use `--dry-run` first. A key that was ever in a chat is exposed
to what read that chat: purge removes the plaintext copies, **rotate the key** if it matters.

## The guard

`integrations/claude/hooks/secret-guard.sh` is a `UserPromptSubmit` hook: it reads the prompt JSON on stdin, looks for key formats (Anthropic, OpenAI-style, GitHub, AWS, Stripe live, CircleCI,
Linear, Slack, Google, private keys, `VARIABLE_KEY=value`) and prints only the kind it found. `SECRET_GUARD=off|warn|block` (environment, else `bhote config set SECRET_GUARD …`, default `warn`):
`warn` exits 0 with a `systemMessage` for the person (the prompt is sent), `block` exits 2 (Claude Code's blocking code: the prompt is not processed and erased, the stderr text is shown to the
person). bhote does not edit your settings; add it yourself, for example to `settings.json`:

```json
{ "hooks": { "UserPromptSubmit": [ { "hooks": [ { "type": "command", "command": "~/Development/bhote/integrations/claude/hooks/secret-guard.sh" } ] } ] } }
```

## Threat model

Protects against: keys ending up in chats, transcripts, history, shell snapshots and logs; keys in the arguments of processes; keys in repositories (environments hold references).
Does **not** protect against a local attacker or malware running as you: the Keychain and `secret-tool` hand the value to any process of your user that is allowed to ask (the first access may ask you),
and the encrypted-file fallback keeps its key next to the files (it protects against a backup, a `grep` or a stray `cat`, not against someone who can read both). `--reveal` is refused inside a Claude Code
session (`CLAUDECODE` set) as a guard against accidents; an agent that runs arbitrary code as you can still ask the store itself. `env run` puts the value in the environment of the command, which
the command and its children can read.

## 1Password

`bhote secret set NAME --op op://vault/item/field` stores the reference only. It resolves with `op read` (needs `op signin`; the desktop app integration answers with a prompt on first use). Moving keys between
machines is `op`'s job, not bhote's.
