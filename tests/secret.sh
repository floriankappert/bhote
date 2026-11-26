#!/usr/bin/env bash
# Tests of the secret manager (bhote secret, bhote env, the chat guard): fake `security` (Keychain), `secret-tool`, `op` and `hc`; every value is an obvious fake.
# Usage: bash tests/secret.sh
cd "$(dirname "$0")/.." || exit 1
unset CLAUDECODE CLAUDE_CODE_SESSION_ID HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID
T=$(mktemp -d); export NO_COLOR=1 TMPDIR=$T PYTHONDONTWRITEBYTECODE=1 BHOTE_CONFIG=$T/cfg/config BHOTE_DATA=$T/data BHOTE_SHARED=$T/shared
mkdir -p "$T/bin" "$T/cfg"; export PATH="$T/bin:$PATH" FAKE_KC=$T/kc FAKE_ST=$T/st FAKE_OP=$T/op; mkdir -p "$FAKE_KC" "$FAKE_ST" "$FAKE_OP"
cat > "$T/bin/security" <<'PY'
#!/usr/bin/env python3
import os, shlex, sys
d = os.environ["FAKE_KC"]
def run(argv):
    cmd, a = argv[0], argv[1:]
    acct = a[a.index("-a") + 1] if "-a" in a else ""
    f = os.path.join(d, acct)
    if cmd == "add-generic-password": open(f, "w").write(a[a.index("-w") + 1]); return 0
    if cmd == "find-generic-password":
        if not os.path.exists(f): return 44
        print(open(f).read()); return 0
    if cmd == "list-keychains": return 1 if os.environ.get("FAKE_NO_KC") else (print('"/fake/login.keychain-db"') or 0)
    if cmd == "delete-generic-password":
        if os.path.exists(f): os.unlink(f)
        return 0
    return 1
if sys.argv[1:2] == ["-i"]: sys.exit(run(shlex.split(sys.stdin.readline())))
sys.exit(run(sys.argv[1:]))
PY
cat > "$T/bin/secret-tool" <<'SH'
#!/bin/sh
d=${FAKE_ST:?}
case "$1" in
  store) while [ $# -gt 0 ]; do [ "$1" = name ] && n=$2; shift; done; cat > "$d/${n:?}" ;;
  lookup) while [ $# -gt 0 ]; do [ "$1" = name ] && n=$2; shift; done; [ -f "$d/${n:?}" ] && cat "$d/$n" || exit 1 ;;
  clear) while [ $# -gt 0 ]; do [ "$1" = name ] && n=$2; shift; done; rm -f "${d:?}/${n:?}" ;;
esac
SH
cat > "$T/bin/op" <<'SH'
#!/bin/sh
# op read --no-newline op://vault/item/field
for u; do :; done
f=${FAKE_OP:?}/$(printf %s "$u" | tr '/:' '__'); [ -f "$f" ] && cat "$f" || exit 1
SH
chmod +x "$T/bin/security" "$T/bin/secret-tool" "$T/bin/op"
ROOT=$PWD; B() { bash "$ROOT/bhote" "$@"; }
fail=0; ok() { echo "ok   $1"; }; bad() { echo "FAIL $1"; fail=1; }
fp() { printf %s "$1" | python3 -c 'import sys,hashlib;print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest()[:10])'; }
mode() { stat -f %Lp "$1" 2>/dev/null || stat -c %a "$1"; }
# obviously fake values
V1=sk-ant-FAKEFAKEFAKEFAKEFAKEFAKE0123456789-FAKEEND   # 49 characters
GH=ghp_FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE1234
AW=AKIAFAKEFAKEFAKE1234
PW=Zq.Passw0rd-xyz
CC=CCIPAT_FAKEFAKEFAKEFAKEFAKEFAKE_1234
leak() { grep -rqaF -e "$V1" -e "$GH" -e "$AW" -e "$PW" -e "$CC" "$1" 2>/dev/null; }   # does any value show up in what was printed or stored?

export BHOTE_SECRET_BACKEND=keychain

# 1) keychain: set from stdin, ls/get never show the value, masked form first 6 + last 3 + length
printf '%s\n' "$V1" | B secret set ANTHROPIC_API_KEY --stdin > "$T/o1" 2>&1
grep -q "ANTHROPIC_API_KEY: stored in keychain" "$T/o1" && ok "set --stdin: stored in the (fake) keychain" || bad "set: $(cat "$T/o1")"
[ "$(cat "$FAKE_KC/ANTHROPIC_API_KEY")" = "$V1" ] && ok "the value is in the keychain" || bad "keychain content"
B secret ls > "$T/o2"; B secret ls --json > "$T/o3"; B secret get ANTHROPIC_API_KEY > "$T/o4"
if leak "$T/o1" || leak "$T/o2" || leak "$T/o3" || leak "$T/o4"; then bad "a value shows in set/ls/get output"; else ok "set, ls, ls --json and get never print the value"; fi
grep -qF "sk-ant…END (49)" "$T/o2" && ok "masked: first 6, last 3, length (sk-ant…END (49))" || bad "mask format: $(cat "$T/o2")"
if leak "$BHOTE_CONFIG" || leak "$T/cfg/secrets.json"; then bad "a value in the config or the index"; else ok "the index holds names, masked form and fingerprint, no value"; fi
[ "$(python3 -c 'import json;print(json.load(open("'"$T"'/o3"))["secrets"][0]["fp"])')" = "$(fp "$V1")" ] && ok "fingerprint is the first 10 hex of sha256" || bad "fingerprint"
[ "$(mode "$T/cfg/secrets.json")" = 600 ] && ok "index is 0600" || bad "index mode"

# 2) verify, tamper, reveal rules
B secret verify ANTHROPIC_API_KEY | grep -q "ok" && ok "verify: readable and fingerprint matches" || bad "verify"
printf 'something-else-entirely' > "$FAKE_KC/ANTHROPIC_API_KEY"
B secret verify ANTHROPIC_API_KEY >/dev/null 2>&1; [ $? = 1 ] && ok "verify fails when the stored value changed" || bad "verify tamper"
printf '%s' "$V1" > "$FAKE_KC/ANTHROPIC_API_KEY"
[ "$(B secret get ANTHROPIC_API_KEY --reveal)" = "$V1" ] && ok "get --reveal prints the value" || bad "reveal"
CLAUDECODE=1 B secret get ANTHROPIC_API_KEY --reveal > "$T/o5" 2>&1; rc=$?
if [ $rc != 0 ] && ! leak "$T/o5"; then ok "--reveal is refused inside an agent session (and prints nothing)"; else bad "reveal guard"; fi

# 3) --from-file --shred
printf '%s\n' "$GH" > "$T/f1"
B secret set GITHUB_TOKEN --from-file "$T/f1" --shred >/dev/null 2>&1
if [ ! -e "$T/f1" ] && [ "$(cat "$FAKE_KC/GITHUB_TOKEN")" = "$GH" ]; then ok "--from-file --shred: stored, the file is gone"; else bad "shred"; fi
printf '%s\n' "$PW" > "$T/f2"; B secret set KEEP_ME --from-file "$T/f2" >/dev/null 2>&1; [ -e "$T/f2" ] && ok "--from-file without --shred keeps the file" || bad "no shred"
B secret set X </dev/null >/dev/null 2>&1; [ $? = 1 ] && ok "set without a terminal and without --stdin/--from-file: error, nothing stored" || bad "set no tty"

# 4) rename / rm
B secret rename GITHUB_TOKEN GH_TOKEN >/dev/null
if [ "$(cat "$FAKE_KC/GH_TOKEN")" = "$GH" ] && [ ! -e "$FAKE_KC/GITHUB_TOKEN" ]; then ok "rename moves the value"; else bad "rename"; fi
B secret rm GH_TOKEN >/dev/null
if [ ! -e "$FAKE_KC/GH_TOKEN" ] && ! B secret ls | grep -q GH_TOKEN; then ok "rm removes value and entry"; else bad "rm"; fi
B secret rm KEEP_ME >/dev/null

# 5) secret-tool and the encrypted file
printf '%s\n' "$AW" | BHOTE_SECRET_BACKEND=secret-tool B secret set AWS_X --stdin >/dev/null 2>&1
if [ "$(cat "$FAKE_ST/AWS_X")" = "$AW" ] && B secret verify AWS_X | grep -q ok; then ok "secret-tool backend: store and verify"; else bad "secret-tool"; fi
printf '%s\n' "$PW" | BHOTE_SECRET_BACKEND=file B secret set FILE_PW --stdin >/dev/null 2>&1
if B secret verify FILE_PW | grep -q ok && ! grep -qaF "$PW" "$T/cfg/secrets.d/FILE_PW.enc" && [ "$(mode "$T/cfg/secrets.d/FILE_PW.enc")" = 600 ]; then ok "file backend: encrypted, 0600, verify ok"; else bad "file backend"; fi
B secret ls | grep -q "FILE_PW .*file" && ok "ls shows the backend per secret" || bad "ls backends"

# 6) 1Password reference: only the URI is stored, resolved at use time
printf '%s' "$PW" > "$FAKE_OP/op___vault_item_field"
B secret set OP_PW --op op://vault/item/field >/dev/null 2>&1
if grep -q 'op://vault/item/field' "$T/cfg/secrets.json" && ! leak "$T/cfg/secrets.json" && [ "$(B secret get OP_PW --reveal)" = "$PW" ]; then ok "op:// reference: URI stored, value resolved through op"; else bad "op ref"; fi
B secret verify OP_PW | grep -q ok && ok "verify works for a reference" || bad "op verify"
B secret rm OP_PW >/dev/null; B secret rm AWS_X >/dev/null; B secret rm FILE_PW >/dev/null

# 7) scan: real secrets found (masked), false positives left out
C=$T/claude; mkdir -p "$C/projects/-demo" "$C/shell-snapshots"; export BHOTE_CLAUDE_DIRS=$C
{
  printf '{"type":"user","message":"export ANTHROPIC_API_KEY=%s and GH %s"}\n' "$V1" "$GH"
  printf '{"type":"tool","message":"aws key %s"}\n' "$AW"
  printf '{"type":"user","message":"HOMGAR_PASSWORD=%s"}\n' "$PW"
  printf '{"type":"tool","message":"HERDR_SOCKET_PATH=/tmp/herdr/some/socket.sock PW_CHROME_PATH=/Applications/Chrome.app/x"}\n'
  printf '{"type":"tool","message":"NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY=pk_test_FAKEFAKEFAKEFAKEFAKEFAKE1234 STRIPE=pk_live_FAKEFAKEFAKEFAKEFAKE1234"}\n'
  printf '{"type":"tool","message":"OPENAI_API_KEY=your-api-key-here-1234567890 and ANTHROPIC_API_KEY=sk-ant-api03-... GITHUB_TOKEN=${GITHUB_TOKEN}"}\n'
} > "$C/projects/-demo/aaaa.jsonl"
printf 'export CIRCLECI_TOKEN=%s\n' "$CC" > "$C/shell-snapshots/snap-1.sh"
touch -t 202601010000 "$C/projects/-demo/aaaa.jsonl" "$C/shell-snapshots/snap-1.sh"
B secret scan --json > "$T/scan.json"
n=$(python3 -c 'import json;print(len(json.load(open("'"$T"'/scan.json"))["found"]))')
[ "$n" = 5 ] && ok "scan: the 5 real secrets (anthropic, github, aws, password, circleci), no path, no pk_ key, no placeholder" || bad "scan found $n: $(cat "$T/scan.json")"
leak "$T/scan.json" && bad "scan prints a value" || ok "scan --json prints masked forms only"
grep -q 'pk_' "$T/scan.json" && bad "publishable key listed" || ok "publishable keys are filtered"
B secret scan > "$T/scan.txt"; if leak "$T/scan.txt"; then bad "scan text leaks"; elif grep -q "NOT in the store" "$T/scan.txt"; then ok "scan text: masked, says what is not in the store"; else bad "scan text"; fi

# 8) import --from-scan: names, store, read-back verification
B secret import --from-scan > "$T/imp" 2>&1; rc=$?
leak "$T/imp" && bad "import prints a value" || ok "import prints masked forms only"
if [ $rc = 0 ] && [ "$(grep -c verified "$T/imp")" = 5 ]; then ok "import: 5 stored and verified by reading them back"; else bad "import rc=$rc: $(cat "$T/imp")"; fi
B secret ls > "$T/names"
if grep -q "^ANTHROPIC_API_KEY " "$T/names" && grep -q "^HOMGAR_PASSWORD " "$T/names" && grep -q "^GITHUB_TOKEN_$(fp "$GH" | cut -c1-4) " "$T/names" && grep -q "^AWS_ACCESS_KEY_ID_$(fp "$AW" | cut -c1-4) " "$T/names" && grep -q "^CIRCLECI_TOKEN " "$T/names"; then
    ok "names: variable name, else KIND_<fp4>"; else bad "names: $(cat "$T/names")"; fi
[ "$(B secret import --from-scan | grep -c "already stored")" = 5 ] && ok "a second import changes nothing" || bad "import twice"
B secret verify --all | grep -vq "ok" && bad "verify --all" || ok "verify --all: everything ok"

# 9) purge-context
B secret purge-context --dry-run > "$T/dry" 2>&1
leak "$T/dry" && bad "dry run leaks" || ok "purge dry run prints counts only"
grep -q "would replace" "$T/dry" && grep -qF "$V1" "$C/projects/-demo/aaaa.jsonl" && ok "dry run: nothing changed" || bad "dry: $(cat "$T/dry")"
chmod 640 "$C/projects/-demo/aaaa.jsonl"
printf '%s\n' '{"type":"user","message":"fresh '"$GH"'"}' > "$C/projects/-demo/fresh.jsonl"      # modified just now: a session is writing it
printf '%s\n' "broken line without json $AW {" >> "$C/projects/-demo/aaaa.jsonl"; touch -t 202601010000 "$C/projects/-demo/aaaa.jsonl"
B secret purge-context > "$T/pur" 2>&1; rc=$?
leak "$T/pur" && bad "purge prints a value" || ok "purge prints masked counts only"
if [ $rc = 0 ] && ! grep -qF -e "$V1" -e "$PW" -e "$GH" "$C/projects/-demo/aaaa.jsonl" && grep -q "«secret:ANTHROPIC_API_KEY#$(fp "$V1")»" "$C/projects/-demo/aaaa.jsonl"; then ok "purge: values replaced by «secret:NAME#fp»"; else bad "purge: $(cat "$T/pur")"; fi
python3 - "$C/projects/-demo/aaaa.jsonl" <<'PY' && ok "every rewritten JSONL line is still valid JSON; the broken line was kept as it was" || bad "jsonl validity"
import json, sys
lines = open(sys.argv[1], encoding="utf-8").read().splitlines()
good = 0
for l in lines:
    try: json.loads(l); good += 1
    except ValueError: assert l.startswith("broken line"), l
assert good == 6 and len(lines) == 7, (good, len(lines))
PY
grep -qF "$GH" "$C/projects/-demo/fresh.jsonl" && ok "a file modified in the last 15 minutes is skipped" || bad "fresh file purged"
[ "$(mode "$C/projects/-demo/aaaa.jsonl")" = 640 ] && ok "purge keeps the file mode" || bad "mode lost"
[ "$(python3 -c 'import os;print(int(os.path.getmtime("'"$C"'/projects/-demo/aaaa.jsonl")))')" = "$(python3 -c 'import time;print(int(time.mktime((2026,1,1,0,0,0,0,0,-1))))')" ] && ok "purge keeps the modification time" || bad "mtime changed"
grep -qF "$CC" "$C/shell-snapshots/snap-1.sh" && bad "snapshot not purged" || ok "shell snapshot purged too"
# only verified secrets: a tampered one stays where it is
printf '%s\n' '{"m":"again '"$GH"'"}' > "$C/projects/-demo/bbbb.jsonl"; touch -t 202601010000 "$C/projects/-demo/bbbb.jsonl"
gn=$(B secret ls | awk '/^GITHUB_TOKEN_/{print $1}'); printf 'different' > "$FAKE_KC/$gn"
B secret purge-context > "$T/pur2" 2>&1
if grep -qF "$GH" "$C/projects/-demo/bbbb.jsonl" && grep -q "not verified" "$T/pur2"; then ok "an unverifiable secret is not purged (and reported)"; else bad "unverified purge: $(cat "$T/pur2")"; fi
printf '%s' "$GH" > "$FAKE_KC/$gn"
B secret purge-context >/dev/null 2>&1; grep -qF "$GH" "$C/projects/-demo/bbbb.jsonl" && bad "second run" || ok "a later run purges once it verifies"
unset BHOTE_CLAUDE_DIRS

# 10) environments
B secret set ENV_SEC --stdin <<< "$PW" >/dev/null 2>&1
B env set web API_TOKEN=ENV_SEC >/dev/null && ok "env set: a variable points at a secret" || bad "env set"
grep -qF "$PW" "$T/cfg/envs.json" && bad "env file leaks" || ok "envs.json holds references, not values"
[ "$(B env run web -- sh -c 'printf %s "$API_TOKEN"')" = "$PW" ] && ok "env run: the command gets the variable" || bad "env run"
B env set bad X=NOPE >/dev/null 2>&1; [ $? = 1 ] && ok "env set: unknown secret refused" || bad "env unknown"
mkdir -p "$T/proj/sub"; B env bind web "$T/proj" >/dev/null
[ "$(cd "$T/proj/sub" && B env run --auto -- sh -c 'printf %s "$API_TOKEN"')" = "$PW" ] && ok "env run --auto: the environment bound to the folder" || bad "env auto"
[ "$(cd "$T" && B env run --auto -- sh -c 'printf %s "x${API_TOKEN}"')" = "x" ] && ok "env run --auto: nothing bound, the command runs as it is" || bad "env auto none"
B secret rename ENV_SEC ENV_SEC2 >/dev/null; B env ls | grep -q "API_TOKEN=ENV_SEC2" && ok "rename keeps the environments pointing right" || bad "env follow rename"

# 11) the bhote config: import, secret:NAME, circleci_token
printf 'CIRCLECI_TOKEN=%s\n' "FAKEcircleTokenFAKE0123456789" > "$BHOTE_CONFIG"
B secret import --config > "$T/ic" 2>&1
if grep -q "CIRCLECI_TOKEN .*imported" "$T/ic" && ! grep -q "FAKEcircleTokenFAKE0123456789" "$T/ic"; then ok "import --config: the config secret is in the store"; else bad "import config: $(cat "$T/ic")"; fi
B config set CIRCLECI_TOKEN secret:CIRCLECI_TOKEN >/dev/null
[ "$(BHOTE_ALLOW_REVEAL=1 BHOTE_SOURCE_ONLY=1 bash -c '. ./bhote; circleci_token')" = "FAKEcircleTokenFAKE0123456789" ] && ok "circleci_token follows secret:NAME into the store" || bad "circleci_token via store"
B config get CIRCLECI_TOKEN | grep -q "(set)" && ok "bhote config never prints it" || bad "config get"

# 12) machine view (fake hc: a second machine with its own home)
export FAKE_RHOME=$T/rhome; mkdir -p "$FAKE_RHOME"
cat > "$T/bin/hc" <<'SH'
#!/bin/sh
case "$1 $2" in
  "machine ls") echo '{"machines":[{"name":"box","state":"connected"}]}' ;;
  "exec --on") shift 3; HOME=$FAKE_RHOME BHOTE_CONFIG=$FAKE_RHOME/cfg/config sh -c "$1" ;;
  "file write") p=$5; mkdir -p "$(dirname "$p")"; cat > "$p" ;;
esac
SH
chmod +x "$T/bin/hc"
export BHOTE_SECRET_BACKEND=file
(export HOME=$FAKE_RHOME BHOTE_CONFIG=$FAKE_RHOME/cfg/config; printf '%s\n' "$PW" | python3 herdr-app/mods/bhote/secretstore.py set ENV_SEC2 --stdin >/dev/null; printf 'other-fake-value-9876543210\n' | python3 herdr-app/mods/bhote/secretstore.py set ONLY_THERE --stdin >/dev/null)
B secret rm ENV_SEC2 >/dev/null; printf '%s\n' "$PW" | B secret set ENV_SEC2 --stdin >/dev/null
B secret machines --json > "$T/mx.json"
python3 - "$T/mx.json" <<'PY' && ok "machine view: present / missing per machine, read-only" || bad "machines: $(cat "$T/mx.json")"
import json, sys
d = json.load(open(sys.argv[1])); assert len(d["machines"]) == 2
r = {x["name"]: x["machines"] for x in d["secrets"]}
st = lambda n: sorted(c["state"] for c in r[n].values())
assert st("ENV_SEC2") == ["present", "present"], st("ENV_SEC2")
assert st("ONLY_THERE") == ["missing", "present"], st("ONLY_THERE")
PY
leak "$T/mx.json" && bad "machine view leaks" || ok "machine view carries no value"
B secret scan --machine box --json | python3 -c 'import json,sys;json.load(sys.stdin)' && ok "scan --machine runs the scanner on the other machine" || bad "scan --machine"

# 13) the chat guard (UserPromptSubmit hook)
G=integrations/claude/hooks/secret-guard.sh
printf '{"prompt":"my key %s"}' "$GH" | SECRET_GUARD=block bash "$G" > "$T/g1" 2> "$T/g2"; rc=$?
if [ $rc = 2 ] && grep -q "Bitte nicht in den Chat" "$T/g2" && grep -q "bhote secret set NAME" "$T/g2" && ! leak "$T/g2"; then ok "guard block: exit 2, German notice, no value"; else bad "guard block rc=$rc"; fi
printf '{"prompt":"my key %s"}' "$GH" | SECRET_GUARD=warn bash "$G" > "$T/g3"; rc=$?
if [ $rc = 0 ] && grep -q systemMessage "$T/g3" && ! leak "$T/g3"; then ok "guard warn: exit 0, notice only for the person"; else bad "guard warn"; fi
printf '{"prompt":"my key %s"}' "$GH" | SECRET_GUARD=off bash "$G"; [ $? = 0 ] && ok "guard off: silent" || bad "guard off"
printf '{"prompt":"nothing special, FOO_KEY=your-key-here"}' | SECRET_GUARD=block bash "$G"; [ $? = 0 ] && ok "guard: no key, no stop" || bad "guard false positive"

# 14) the broker: a process that cannot reach the keychain (FAKE_NO_KC) has its command run by the broker of the login session
SP=herdr-app/mods/bhote/secretstore.py; export BHOTE_BROKER_DIR=$T/broker
printf 'brk-value-4711' | python3 -I "$SP" set BRK_TEST --stdin >/dev/null 2>&1
echo "SECRET_BROKER_ALLOW=env,git" >> "$BHOTE_CONFIG"
python3 -I "$SP" broker run >/dev/null 2>&1 & BP=$!; sleep 1.5
n=$(FAKE_NO_KC=1 python3 -I "$SP" env run -- env 2>/dev/null | grep -c '^BRK_TEST=brk-value-4711$'); [ "$n" = 1 ] && ok "broker: the command runs with the secret in its environment" || bad "broker run ($n)"
FAKE_NO_KC=1 python3 -I "$SP" env run -- sh -c 'echo hi' > "$T/b1" 2> "$T/b2"; rc=$?
[ $rc = 126 ] && ! grep -q '^hi' "$T/b1" && grep -q "not allowed" "$T/b2" && ok "broker: a program outside SECRET_BROKER_ALLOW is refused" || bad "broker allow list ($rc)"
FAKE_NO_KC=1 python3 -I "$SP" env run -- git -c core.pager=x status >/dev/null 2>&1; [ $? = 126 ] && ok "broker: git -c is refused" || bad "broker git -c"
FAKE_NO_KC=1 python3 -I "$SP" env run -- git nosuchcmd >/dev/null 2>&1; [ $? = 1 ] && ok "broker: the exit code of the command comes back" || bad "broker exit code"
kill $BP 2>/dev/null; wait $BP 2>/dev/null
grep -q "refused sh" "$BHOTE_DATA/secret-broker.log" && grep -q "run env" "$BHOTE_DATA/secret-broker.log" && grep -q "done env rc=0" "$BHOTE_DATA/secret-broker.log" && ! grep -q "brk-value-4711" "$BHOTE_DATA/secret-broker.log" && ok "broker: every request is logged in bhote's data folder, without a value" || bad "broker log"
touch -t 200001010000 "$BHOTE_BROKER_DIR/alive"      # (a broker that stopped a while ago)
FAKE_NO_KC=1 python3 -I "$SP" env run -- git --version >/dev/null 2> "$T/b3"; grep -q "no broker runs" "$T/b3" && ok "broker: without one the old behaviour stays, with a hint" || bad "no broker hint"
unset BHOTE_BROKER_DIR

rm -rf "${T:?}"; exit $fail
