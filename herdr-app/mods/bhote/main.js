// bhote for herdr-app: the topics and agents panels as a mod (bhote 0.9.3 split layout).
// props().side "left":  the workspaces, then the cards (review, now, testing, deploying, finished; an agent without
//                       a card has one of its own: idle, working, needs you).
// props().side "right": the lines of the topics (next, waiting, later), the last done ones, the deployments and tests.
// Data: agents() / spaces() of herdr-app (live), the topic files and the monitor lists bhoted writes (files are the
// interface). Actions: the bhote CLI through exec(), herdr-app commands through run().

import { View, div, svg, PathBuilder, Background } from "gpui-kit";
import { v_flex, h_flex, InputState, Input, TextareaState, Textarea } from "gpui-base";
import { agents, spaces, run, theme, props, exec, home, watchFolder, shared, setShared, openOverlay, closeOverlay, machines, settings, readTextFile, window, focused } from "herdr";

const STATUSES = new Set(["now", "testing", "finished", "next", "waiting", "review", "later", "done"]);
const CARD_ORDER = ["review", "now", "testing", "deploying", "finished"];
const SPIN = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// The text of all topic files, each one after a record separator (0x1e) line, as topics. The first value of a key
// counts (as in bhote's own index); deleted records and other kinds are skipped.
function parseTopics(text) {
  const out = [];
  for (const block of text.split("\x1e\n")) {
    const v = {};
    for (const line of block.split("\n")) {
      const i = line.indexOf("=");
      if (i > 0 && !(line.slice(0, i) in v)) v[line.slice(0, i)] = line.slice(i + 1);
    }
    if (!v.id || v.deleted === "1" || v.kind) continue;
    const status = STATUSES.has(v.status) ? v.status : "now";
    out.push({
      id: v.id,
      title: v.title ?? "",
      desc: v.description ?? "",
      status,
      // a waiting topic that follows a deployment is a card of its own ("deploying")
      group: status === "waiting" && v.deploy_wait === "1" ? "deploying" : status,
      waiting_for: v.waiting_for || null,
      updated: Number(v.updated) || 0,
      created: Number(v.created) || 0,
      since: Number(v.waiting_since) || 0,
      dismissed: v.dismissed === "1",
      agent: v.agent || null,
      machine: v.agent_machine || "",
      branch: v.agent_branch || "",
      project: v.project || "",
    });
  }
  return out.sort((a, b) => b.updated - a.updated);
}

// A progress bar: a track and a fill (a percentage of the track).
function progress(t, frac, color, width) {
  const pct = Math.max(0, Math.min(100, Math.round(frac * 100)));
  return div().h(5).w(width ?? 110).flex_none().rounded(3).bg(t.border).overflow_hidden().child(div().h(5).w(`${pct}%`).rounded(3).bg(color));
}

// "~4m left", "~1h 5m left", "<1m left"
function left(sec) {
  if (sec == null) return "";
  if (sec < 60) return "<1m left";
  if (sec < 3600) return `~${Math.round(sec / 60)}m left`;
  return `~${Math.floor(sec / 3600)}h ${Math.round((sec % 3600) / 60)}m left`;
}

const median = (a) => { const b = [...a].sort((x, y) => x - y); return b.length ? b[Math.floor(b.length / 2)] : null; };

// A monitor list: time US project US label US state US done US total US note ("branch|~|title") US url.
const FLASH_MS = 1200, FADE_MS = 1800;   // the light of a finished card dies down over the first 1.2 s; it fades to idle over the last 1.8 s
const FINISH_MS = 15000;   // a recently finished agent: lit up for a moment, "finished" until here, then idle

// While a menu is open, what lies behind it must not light up under the pointer (the popup does not catch the pointer for the panes): every
// hover style is skipped then. The menu itself (its own instance) keeps its hover.
{
  const open = () => {
    try { if (props()?.menu) return false; const at = shared("bhote.menuOpenAt") ?? 0; return at > 0 && Date.now() - at < 120000; } catch (_e) { return false; }
  };
  for (const proto of new Set([div(), h_flex(), v_flex()].map((x) => Object.getPrototypeOf(x)))) {
    const hover = proto.hover;
    if (typeof hover === "function" && !hover.__bhote) { const w = function (fn) { return hover.call(this, (e) => (open() ? e : fn(e))); }; w.__bhote = true; proto.hover = w; }
  }
}

function parseMonitor(text) {
  return text.split("\n").filter(Boolean).map((l) => {
    const f = l.split("\x1f");
    const note = f[6] ?? "";
    const [branch, title] = note.includes("|~|") ? note.split("|~|") : ["", note];   // a test note is "branch|~|title", a deployment's the title alone
    return { time: Number(f[0]) || 0, project: f[1] ?? "", label: f[2] ?? "", state: f[3] ?? "", done: Number(f[4]) || 0, total: Number(f[5]) || 0, branch: branch ?? "", title: title ?? "", url: f[7] ?? "" };
  });
}

function ago(sec) {
  const d = Math.max(0, Date.now() / 1000 - sec);
  if (d < 60) return "just now";
  if (d < 3600) return `${Math.floor(d / 60)}m ago`;
  if (d < 172800) return `${Math.floor(d / 3600)}h ago`;
  return `${Math.floor(d / 86400)}d ago`;
}

function clock(sec) {
  const t = new Date(sec * 1000), n = new Date();
  const hm = `${String(t.getHours()).padStart(2, "0")}:${String(t.getMinutes()).padStart(2, "0")}`;
  return t.toDateString() === n.toDateString() ? hm : `${t.getDate()}.${t.getMonth() + 1}. ${hm}`;
}

// "■■■□□ 19%" as bhote draws its small bars
function bar(done, total) {
  if (total <= 0) return "";
  const f = Math.min(5, Math.floor((done * 5) / total + 0.5));
  return `${"■".repeat(f)}${"□".repeat(5 - f)} ${Math.floor((done * 100) / total)}%`;
}

// bhote names an agent after its old herdr workspace ("IMS (wt2)"), herdr-app after the folder ("IMS-wt2"): compare letters and digits only.
const norm = (s) => (s ?? "").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");

// The names of the agents: a name from Middle-earth instead of w2:a1. An agent has a number that is the same on every machine (bhote.agentnums in
// kharka, the record of its machine and name) and counts per machine, so the name comes from the rank of its record among all of them (by the time they
// were made, deleted ones count too: no name moves). Its name is the one at that place of this list, from the 61st on with a count behind it (Frodo2).
// Short, letters only, easy to type. The same list is RING_NAMES in the bhote script (a test keeps them equal).
const RING = [
  "Frodo", "Sam", "Merry", "Pippin", "Bilbo", "Gandalf", "Aragorn", "Legolas", "Gimli", "Boromir", "Elrond", "Arwen", "Gollum", "Sauron", "Saruman", "Theoden", "Eowyn", "Eomer", "Faramir", "Haldir",
  "Thorin", "Balin", "Dwalin", "Fili", "Kili", "Bofur", "Bombur", "Dori", "Nori", "Ori", "Oin", "Gloin", "Smaug", "Beorn", "Bard", "Tauriel", "Shelob", "Grima", "Isildur", "Elendil", "Rosie", "Tom",
  "Gwaihir", "Elladan", "Elrohir", "Azog", "Bolg", "Imrahil", "Hama", "Gamling", "Rohan", "Gondor", "Mordor", "Moria", "Lorien", "Erebor", "Bree", "Shire", "Dale", "Rumil",
];
// The names of all agents seen in the topic records: normalised "machine/name" -> its name. Same order and same tie-break as ring_load in the bhote script.
function parseRing(text) {
  const recs = [];
  for (const block of text.split("\x1e\n")) {
    const v = {};
    for (const line of block.split("\n")) { const i = line.indexOf("="); if (i > 0 && !(line.slice(0, i) in v)) v[line.slice(0, i)] = line.slice(i + 1); }
    if (v.kind === "agentnum" && v.title) recs.push({ key: v.title.toLowerCase().replace(/[^a-z0-9]/g, ""), created: Number(v.created) || 0 });
  }
  recs.sort((a, b) => a.created - b.created || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  const map = new Map();
  recs.forEach((r, i) => { if (!map.has(r.key)) map.set(r.key, ringName(i + 1)); });
  return map;
}
function ringName(num) {
  const n = Number(num);
  if (!(n >= 1)) return "";
  const r = Math.floor((n - 1) / RING.length);
  return RING[(n - 1) % RING.length] + (r ? String(r + 1) : "");
}

// The project records (kind=project): their repositories say which GitHub repos to ask for pull requests.
function parseProjects(text) {
  const out = [];
  for (const block of text.split("\x1e\n")) {
    const v = {};
    for (const line of block.split("\n")) {
      const i = line.indexOf("=");
      if (i > 0 && !(line.slice(0, i) in v)) v[line.slice(0, i)] = line.slice(i + 1);
    }
    if (v.kind !== "project" || v.deleted === "1" || !v.id) continue;
    out.push({ id: v.id, title: v.title || v.id, ci: v.ci || "", raw: { ci: v.ci ?? "", deploy_branches: v.deploy_branches ?? "", deploy_names: v.deploy_names ?? "", test_names: v.test_names ?? "", runbook: v.runbook ?? "" }, repos: (v.repos ?? "").split(",").map((x) => x.trim()).filter(Boolean),
      deployBranches: (v.deploy_branches ?? "").split(",").map((e) => e.split("=")[0].trim()).filter(Boolean),
      deployJobs: (v.deploy_names ?? "").split(",").map((e) => e.split("=")[0].trim()).filter(Boolean),
      runbook: (v.runbook ?? "").split(",").map((e) => e.trim()).filter(Boolean),   // the stages after the merge, when the default of the project is not right
      // the environments in the order of deploy_branches: develop=Staging,main=Production -> Staging, Production (a label like the branch is just "Deploy")
      stages: (v.deploy_branches ?? "").split(",").map((e) => e.trim()).filter(Boolean).map((e) => { const [b, l] = e.split("="); return !l || l.trim() === b.trim() ? "Deploy" : l.trim(); }) });
  }
  return out;
}

// The runbook of a project: the whole way a change goes, from the commit to the last stage. Before the merge it is the same everywhere (Commit, PR, Tests,
// Merge into the target branch); after it, the stages of the project (the record's `runbook`, else these defaults; else the environments of its deploy
// branches, else "Deploy"): IMS builds a Docker image and deploys it, Bilendo Marketing has Staging, an Approval in CircleCI and Production, Bilendo is
// released by hand with Capistrano, Bhote CLI ends at the merge. Marketing always goes through develop: the pull request is merged into develop (Staging),
// then develop is merged into main, CircleCI asks for an Approval, and Production follows.
// (the stages after the merge are the project's own `runbook`, set in Settings › Runbooks: e.g. IMS "Build (Docker), Deploy", Bilendo Marketing "Staging, Merge (main),
// Approval, Production", Bilendo "Release (Capistrano)"; "none" ends the path at the merge)
function postStages(proj) {
  if (proj?.runbook?.length) return proj.runbook.length === 1 && proj.runbook[0].toLowerCase() === "none" ? [] : proj.runbook;
  if (proj && !proj.ci) return [];   // (no CI: the path ends at the merge)
  return proj?.stages?.length ? proj.stages : ["Deploy"];
}

// o: hasPR, tests (ok | failed | running | none | waiting), merged, target (the branch it is merged into), off (deployments not watched),
//    builds (the builds of the merged pull request: { state }), at / now (a run on a deploy branch: the post stage it is at, and its state)
function runbookStages(proj, o) {
  const stages = [{ name: "Commit", state: "ok" }, { name: "PR", state: o.hasPR ? "ok" : "ready" }, { name: "Tests", state: o.tests },
    { name: "Merge", target: o.target || "", state: o.merged ? "ok" : o.hasPR && o.tests === "ok" ? "ready" : "waiting" }];
  const bs = o.builds ?? [];
  const post = postStages(proj);
  const kind = (l) => l.split(" (")[0];
  const pending = post.findIndex((l) => kind(l) === "Approval") >= 0 && bs.some((b) => b.state === "approval") ? post.findIndex((l) => kind(l) === "Approval") : -1;
  const busy = bs.some((b) => b.state === "failed") ? "failed" : bs.some((b) => b.state === "running") ? "running" : "";
  const first = post.findIndex((l) => !["Approval", "Release", "Merge"].includes(kind(l)));   // the stage a running or failed build is for
  post.forEach((l, i) => {
    let state = "waiting";
    if (o.at != null && o.at >= 0) state = i < o.at ? "ok" : i === o.at ? o.now : "waiting";   // (a run on a deploy branch)
    else if (o.merged) {
      if (kind(l) === "Release" || o.off) state = kind(l) === "Release" || i === 0 ? "ready" : "waiting";   // (by hand, or not watched: the next step is yours)
      else if (pending >= 0) state = i < pending ? "ok" : i === pending ? "approval" : "waiting";
      else if (busy && i === first) state = busy;
    }
    const m = /^Merge \((.+)\)$/.exec(l);   // (a second merge, e.g. develop into main: Merge → main)
    stages.push(m ? { name: "Merge", target: m[1], state } : { name: l, state });
  });
  return stages;
}

// The stages grouped into phases for the runbooks tile: what belongs closely together is one circle. Commit and PR are the Code phase; the Tests and the
// first Merge stand alone; after the merge the stages up to and including an environment (Staging, Production, Deploy, Release) are one phase named by it:
// Build (Docker) + Deploy = Deploy; Merge into main + Approval + Production = Production. The state of a phase: failed if a stage failed, under way
// (running) if one runs or waits for approval, done when all are done, else open.
function runbookPhases(stages) {
  const ENV = new Set(["Deploy", "Staging", "Production", "Release"]);
  const groups = [];
  const add = (name, note, parts) => groups.push({ name, note, parts });
  const kind = (x) => x.name.split(" (")[0];
  const pre = stages.slice(0, 4), post = stages.slice(4);
  add("Code", "", [pre[0], pre[1]].filter(Boolean));
  if (pre[2]) add("Tests", "", [pre[2]]);
  if (pre[3]) add("Merge", pre[3].target ? `→ ${pre[3].target}` : "", [pre[3]]);
  let cur = [];
  for (const x of post) {
    cur.push(x);
    if (ENV.has(kind(x))) { const note = x.name.includes(" (") ? x.name.slice(x.name.indexOf("(") + 1, -1) : ""; add(kind(x), note, cur); cur = []; }
  }
  if (cur.length) add(kind(cur[cur.length - 1]), "", cur);
  return groups.map((g) => {
    const st = g.parts.map((x) => x.state);
    const state = st.some((x) => x === "failed") ? "failed" : st.some((x) => x === "running" || x === "approval") ? "running" : st.every((x) => x === "ok") ? "ok" : "waiting";
    return { name: g.name, note: g.note, state, parts: g.parts };
  });
}

// A GitHub check of a pull request (a check run of Actions, or a commit status such as CircleCI's) as ok | failed | running | skipped.
function checkOf(c) {
  const name = c.name || c.context || c.workflowName || "check";
  const url = c.detailsUrl || c.targetUrl || "";
  const started = secOf(c.startedAt), ended = secOf(c.completedAt);
  const workflow = c.workflowName || "";
  if (c.__typename === "StatusContext") {
    const st = (c.state ?? "").toUpperCase();
    return { name, url, started, ended, workflow, state: st === "SUCCESS" ? "ok" : st === "FAILURE" || st === "ERROR" ? "failed" : "running" };
  }
  if ((c.status ?? "").toUpperCase() !== "COMPLETED") return { name, url, started, ended: 0, workflow, state: "running" };
  const cc = (c.conclusion ?? "").toUpperCase();
  return { name, url, started, ended, workflow, state: cc === "SUCCESS" ? "ok" : ["SKIPPED", "NEUTRAL"].includes(cc) ? "skipped" : "failed" };
}

// The runs of a branch without a pull request (GitHub Actions) as checks.
function runOf(r) {
  const state = r.status !== "completed" ? "running" : r.conclusion === "success" ? "ok" : ["skipped", "neutral"].includes(r.conclusion) ? "skipped" : "failed";
  return { name: r.workflowName || r.name || "run", url: r.url || "", started: secOf(r.createdAt), ended: 0, state };
}

const PR_FIELDS = "number,title,headRefName,baseRefName,url,isDraft,state,mergedAt,reviewDecision,statusCheckRollup,updatedAt";

const secOf = (iso) => { const n = Date.parse(iso ?? ""); return Number.isFinite(n) ? Math.floor(n / 1000) : 0; };

// This machine's name in bhote's topics: "Mac" on macOS (HOST), else the short hostname.
// A checkbox. Off: an empty rounded square with a clear border. On: filled with the accent, with a drawn check mark. Mixed (some of
// many are ticked): filled, with a dash.
function checkbox(t, on, mixed, muted) {
  const full = on || mixed;
  const color = muted ? t.dim : t.accent;
  return div().w(18).h(18).flex_none().rounded(5).flex().items_center().justify_center()
    .border(1).border_color(full ? color : t.muted).bg(full ? color : t.surface)
    .when(full, (el) => el.child(svg(on ? "mods/bhote/check.svg" : "mods/bhote/minus.svg").size(12).text_color(muted ? t.chrome : (t.accent_foreground ?? "#ffffff"))));
}

// An empty view: a glyph, a title and a hint, in the middle of the space in both directions. `busy` shows a spinner for the glyph.
function empty(t, glyph, title, hint, busy) {
  return v_flex().flex_1().size_full().items_center().justify_center().gap(6).px(24).py(32)
    .child(div().w(52).h(52).rounded(26).bg(t.surface).border(1).border_color(t.border).flex().items_center().justify_center()
      .text_size(22).text_color(t.dim)
      .when(!!busy, (el) => el.frames(SPIN, 90))
      .when(!busy, (el) => el.child(glyph)))
    .child(div().text_size(14).font_semibold().text_color(t.muted).child(title))
    .when(!!hint, (el) => el.child(div().text_size(12).text_color(t.dim).max_w(260).child(hint)));
}

// The accent colour as a translucent ground (a theme colour is #rrggbb): the active card's. Any other form falls back to the selection colour.
const tint = (c, alpha, fallback) => (/^#[0-9a-f]{6}$/i.test(c ?? "") ? `${c}${alpha}` : fallback);

const isLocalName = (m) => !m || m.toLowerCase() === "mac";

// --- services: dev servers that are not IMS (Bilendo, Marketing …), started by a command of the dev shell -----------------------------------------
// The list is the person's own (Settings › Servers, ~/.config/bhote/servers.json): [{ id, name, machine, port, url, start, stop }]. machine is null for this
// computer, else the name of a herdr machine. A start command runs in the dev shell (zsh -ic), detached: it is what you would type (bilendo-start,
// marketing-start), with the output in ~/.cache/bhote/svc/<id>.log and the process group in <id>.pid. Whether it runs is whether its port answers.
const SVC_FILE = "~/.config/bhote/servers.json";
const SVC_SEED = [
  { id: "bilendo", name: "Bilendo", machine: null, port: 3000, url: "http://localhost:3000", start: "bilendo-start", stop: "" },
  { id: "marketing", name: "Bilendo Marketing", machine: null, port: 3002, url: "http://localhost:3002", start: "marketing-start", stop: "marketing-stop" },
];
const SVC_PROBE = '(exec 3<>/dev/tcp/127.0.0.1/"$1") 2>/dev/null && { echo up; exit 0; }; d="${HOME:?}/.cache/bhote/svc"; '   // up | booting (its process lives, the port is not open yet) | down
  + 'if command -v systemctl >/dev/null 2>&1 && systemctl --user is-active --quiet "bhote-svc-$2" 2>/dev/null; then echo booting; exit 0; fi; '
  + 'if [ -f "$d/$2.pid" ] && ps -p "$(cat "$d/$2.pid")" >/dev/null 2>&1; then echo booting; exit 0; fi; echo down';
const SVC_START = 'd="$HOME/.cache/bhote/svc"; mkdir -p "$d"; (exec 3<>/dev/tcp/127.0.0.1/"$2") 2>/dev/null && exit 0; rm -f "$d/$1.pid"; '
  // Linux: a systemd user unit (what a program started through hc exec would not survive); elsewhere a detached process group
  + 'if command -v systemd-run >/dev/null 2>&1; then systemctl --user reset-failed "bhote-svc-$1" 2>/dev/null; '
  + 'systemd-run --user --quiet --collect --unit="bhote-svc-$1" -p StandardOutput=append:"$d/$1.log" -p StandardError=append:"$d/$1.log" zsh -ic "${3//\\$/\\$\\$}"; exit 0; fi; '
  + 'nohup perl -MPOSIX -e \'POSIX::setsid(); exec @ARGV\' sh -c \'echo $$ > "$1"; exec zsh -ic "$2"\' _ "$d/$1.pid" "$3" > "$d/$1.log" 2>&1 < /dev/null & '
  + 'for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do [ -s "$d/$1.pid" ] && break; sleep 0.1; done; exit 0';
const SVC_STOP = 'd="$HOME/.cache/bhote/svc"; [ -n "$3" ] && zsh -ic "$3" >/dev/null 2>&1; systemctl --user stop "bhote-svc-$1" 2>/dev/null; '
  + 'if [ -f "$d/$1.pid" ]; then p=$(cat "$d/$1.pid"); [ "$(ps -o pgid= -p "$p" 2>/dev/null | tr -d " ")" = "$p" ] && { kill -TERM -- "-$p" 2>/dev/null; sleep 2; kill -KILL -- "-$p" 2>/dev/null; }; rm -f "$d/$1.pid"; fi; '
  + 'l=$(lsof -t -nP -iTCP:"$2" -sTCP:LISTEN 2>/dev/null); [ -n "$l" ] || l=$(ss -ltnpH "sport = :$2" 2>/dev/null | sed -n "s/.*pid=\\([0-9]*\\).*/\\1/p" | sort -u); [ -n "$l" ] && kill $l 2>/dev/null; exit 0';

// Test runs that nobody reported: an agent that starts `bin/rails test`, rspec, vitest, jest, pytest or `npm test` in the background does not use
// `bhote test run` and writes no status file, yet it runs for ten minutes. They are found as processes (the top one of each run, its workers do not count),
// with the folder of the repository they run in and its branch. One line each: pid, seconds running, repository folder, branch, command.
const LIVE_TESTS_SH = 'ps -Ao pid=,ppid=,etime=,command= | awk -v re="(rails|rake) +test|bin/rails +test|(^|[ /])rspec( |$)|vitest|(^|[ /])jest( |$)|pytest|(^|[ /])mocha( |$)|(yarn|npm|pnpm)( run)? +test( |$)" \''
  + 'function secs(e,  d,a,n){ d=0; if (index(e,"-")>0){ split(e,a,"-"); d=a[1]; e=a[2] } n=split(e,a,":"); if (n==3) return d*86400+a[1]*3600+a[2]*60+a[3]; return d*86400+a[1]*60+a[2] } '
  + '{ cmd=$0; sub(/^ *[0-9]+ +[0-9]+ +[^ ]+ +/,"",cmd); if (cmd ~ re && cmd !~ /awk|ps -Ao|claude|bhote|hc exec|grep/) { P[$1]=1; PP[$1]=$2; ET[$1]=$3; C[$1]=cmd } } '
  + 'END { for (p in P) if (!(PP[p] in P)) printf "%s\\t%s\\t%s\\n", p, secs(ET[p]), C[p] }\' '
  + '| while IFS="$(printf \'\\t\')" read -r p s c; do cwd=$(readlink "/proc/$p/cwd" 2>/dev/null || lsof -a -p "$p" -d cwd -Fn 2>/dev/null | sed -n "s/^n//p"); '
  + '[ -n "$cwd" ] || continue; top=$(git -C "$cwd" rev-parse --show-toplevel 2>/dev/null || printf %s "$cwd"); br=$(git -C "$top" branch --show-current 2>/dev/null); '
  + 'printf "%s\\t%s\\t%s\\t%s\\t%s\\n" "$p" "$s" "$top" "$br" "$c"; done';

// What the commit inspector asks the repository: the commits of this branch since it left the base branch (origin/HEAD, else main or master), newest first, the
// base commit it left from, and the uncommitted files. On the base branch itself (nothing of its own) the last 10 commits. Never more than the last 10 of a branch; the base line then says how many come before. Plain sh: it runs on every machine.
const COMMITS_SH = 'd="$1"; f="%h%x1f%s%x1f%ct%x1f%an"; b=$(git -C "$d" symbolic-ref -q --short refs/remotes/origin/HEAD 2>/dev/null); '
  + '[ -n "$b" ] || for c in origin/main origin/master main master; do git -C "$d" rev-parse -q --verify "$c" >/dev/null 2>&1 && { b=$c; break; }; done; '
  + 'mb=""; [ -n "$b" ] && mb=$(git -C "$d" merge-base HEAD "$b" 2>/dev/null); h=$(git -C "$d" rev-parse HEAD 2>/dev/null); '
  + 'if [ -n "$mb" ] && [ "$mb" != "$h" ]; then git -C "$d" log -10 --format="$f" "$mb..HEAD"; echo "@@@base"; git -C "$d" log -1 --format="$f" "$mb"; echo "$b"; git -C "$d" rev-list --count "$mb..HEAD"; '
  + 'else git -C "$d" log -10 --format="$f"; echo "@@@base"; fi; echo "@@@status"; git -C "$d" status --porcelain';

// A headless run started by hand: a unit on Linux (a program started through hc exec does not outlive it), a detached process group elsewhere. Arguments: id,
// folder, command (e.g. `claude -p`), prompt, description, the zsh line that runs them. The prompt travels in the environment, never inside a command line.
const HEADLESS_START = 'd="$HOME/.cache/bhote/headless"; mkdir -p "$d"; id="$1"; dir="${2/#\\~/$HOME}"; [ -d "$dir" ] || { echo "no folder $dir" >&2; exit 3; }; printf %s "$5" > "$d/$id.desc"; '
  + 'if command -v systemd-run >/dev/null 2>&1; then systemd-run --user --quiet --collect --unit="bhote-headless-$id" --description="$5" --working-directory="$dir" '
  + '--setenv=BHOTE_CMD="$3" --setenv=BHOTE_PROMPT="$4" -p StandardOutput=append:"$d/$id.log" -p StandardError=append:"$d/$id.log" zsh -ic "${6//\\$/\\$\\$}"; exit $?; fi; '   // (systemd expands $ in a command line itself: $$)
  + 'BHOTE_CMD="$3" BHOTE_PROMPT="$4" nohup perl -MPOSIX -e \'POSIX::setsid(); exec @ARGV\' sh -c \'cd "$3" && echo $$ > "$1" && exec zsh -ic "$2"\' _ "$d/$id.pid" "$6" "$dir" > "$d/$id.log" 2>&1 < /dev/null & '
  + 'for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do [ -s "$d/$id.pid" ] && break; sleep 0.1; done; exit 0';
const HEADLESS_ZSH = 'c=(${(z)BHOTE_CMD}); "${c[@]}" "$BHOTE_PROMPT"';   // (the command split like a shell does, the prompt as one argument)
const HEADLESS_STOP = 'd="${HOME:?}/.cache/bhote/headless"; systemctl --user stop "bhote-headless-$1" 2>/dev/null; systemctl --user reset-failed "bhote-headless-$1" 2>/dev/null; '
  + 'if [ -f "$d/$1.pid" ]; then p=$(cat "$d/$1.pid"); [ "$(ps -o pgid= -p "$p" 2>/dev/null | tr -d " ")" = "$p" ] && { kill -TERM -- "-$p" 2>/dev/null; sleep 2; kill -KILL -- "-$p" 2>/dev/null; }; fi; exit 0';

// The shared folder of the collector, as a sh prefix that sets $s. The daemon of a machine may have been started without XDG_RUNTIME_DIR
// (it then writes to /tmp/bhote-<uid>) while this call runs with it: take the folder whose tests.list is the newest, not the first guess.
const SHARED_SH =
  'u=$(id -u); s=""; for d in "$BHOTE_SHARED" "${XDG_RUNTIME_DIR:+$XDG_RUNTIME_DIR/bhote-$u}" "${TMPDIR:-/tmp}/bhote-$u" "/tmp/bhote-$u"; do ' +
  '[ -n "$d" ] && [ -f "$d/tests.list" ] || continue; if [ -z "$s" ] || [ "$d/tests.list" -nt "$s/tests.list" ]; then s="$d"; fi; done; ' +
  '[ -n "$s" ] || s="${BHOTE_SHARED:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/bhote-$u}"; ';

// --- the settings dialog: every setting of `bhote config list --json`, curated -----------------------------------------------------------
// [key, group, section, name, description, unit, depends-on]. The order here is the order in the dialog. Names are short noun phrases,
// descriptions one sentence ending in a full stop; a unit is drawn at the value. type and default come from the CLI, not from here.
const SETTINGS = [
  ["NOTIFY", "Topics", "", "Notifications", "Notify through herdr when a topic is ready, done or has waited too long."],

  ["WAIT_REMIND", "Topics", "", "Remind waiting topics after", "A waiting topic is reminded once after this time (0 = never).", "h"],
  ["FINISHED_MINUTES", "Topics", "", "Dismiss finished cards after", "An agent that finished without a question drops off the panel after this time (0 = never).", "min"],

  ["AGENT_CARDS", "Agents", "", "Cards for agents without a topic", "A working agent without a topic gets a card on Now after 15 s of work."],
  ["AUTO_REVIEW", "Agents", "", "Review topic for questions", "An agent that waits for an answer gets a review topic with its question."],
  ["AGENT_CAN_CLOSE", "Agents", "", "Agents may close topics", "Off: agents can only hand a topic over for review."],
  ["AGENT_TOPICS", "Agents", "", "Agents may create topics", "Off: bhote add run by an agent creates nothing."],
  ["AGENT_LISTEN", "Agents", "", "Agents listen by default", "Whether agents accept messages from other agents. Each agent's card has its own switch (the ear)."],
  ["AUTO_ASSIGN", "Agents", "", "Auto-assign running topics", "A running topic without an agent goes to the next free agent of its project after 20 s."],
  ["AUTO_ASSIGN_MACHINE", "Agents", "", "Prefer agents on", "Whose free agents come first: any (this machine first) or a machine name.", ""],
  ["TESTS_BUSY", "Agents", "", "Running tests keep an agent busy", "An agent whose workspace runs tests is not free, and its topic stays out of review until they finish."],
  ["LOCAL_EVERY", "Agents", "", "Check local agents every", "How often the agents of this machine are queried.", "s"],

  ["DEPLOY_MONITOR", "Monitors", "Deployments", "Deployment monitor", "Show the latest deployments above the agents."],
  ["DEPLOY_EVERY", "Monitors", "Deployments", "Check every", "How often deployments are checked.", "s"],
  ["DEPLOY_PROJECTS_OFF", "Monitors", "Deployments", "Hidden projects", "Comma-separated projects left out of the deployment monitor.", ""],
  ["TEST_MONITOR", "Monitors", "Tests", "Test monitor", "Show the latest test runs above the agents."],
  ["TEST_EVERY", "Monitors", "Tests", "Check every", "How often test runs are checked.", "s"],
  ["TEST_LOCAL", "Monitors", "Tests", "Local runs", "Include runs of bhote test run and live status files.", ""],
  ["TEST_STATUS_DIR", "Monitors", "Tests", "Status file folder", "Folder with the live status files of running test suites.", ""],
  ["TEST_PROJECTS_OFF", "Monitors", "Tests", "Hidden projects", "Comma-separated projects left out of the test monitor, with their worktrees.", ""],


  ["DEPLOY_GITHUB", "Integrations", "GitHub", "Deployments", "Include deployments from GitHub Actions (needs gh and its login).", ""],
  ["TEST_GITHUB", "Integrations", "GitHub", "Test jobs", "Include test jobs from GitHub Actions.", ""],
  ["GITHUB_BRANCHES", "Integrations", "GitHub", "Agent branches", "Show the git branch each topic's agent works on."],
  ["CIRCLECI_TOKEN", "Integrations", "CircleCI", "API token", "Personal API token for CircleCI (falls back to $CIRCLECI_TOKEN)."],
  ["DEPLOY_CIRCLECI", "Integrations", "CircleCI", "Deployments", "Include deployments from CircleCI.", ""],
  ["DEPLOY_PIPELINES", "Integrations", "CircleCI", "Pipelines on other branches", "Also show CircleCI pipelines running on other branches, with branch and author.", ""],
  ["TEST_CIRCLECI", "Integrations", "CircleCI", "Test jobs", "Include test jobs from CircleCI.", ""],
  ["SLACK_WATCH", "Integrations", "Slack", "Watch pinned channels", "Check the channels pinned to waiting topics and set a topic to review on a new message. One machine is enough."],
  ["SLACK_EVERY", "Integrations", "Slack", "Check every", "How often the pinned channels are checked (one short Claude call each).", "s"],
  ["CLAUDE_MOD", "Integrations", "Claude Code", "Show topic in Claude Code", "Show the agent's topic above the prompt and in the topic pane."],
  ["FRIENDLY_REMINDER", "Integrations", "Claude Code", "Friendly reminder", "Now and then show a waiting, next or parked topic above the prompt.", ""],
  ["CLAUDE_CONFIG_DIR", "Integrations", "Claude Code", "Profile folder", "Profile the Slack watch runs in, e.g. ~/.claude2 (empty = default).", ""],
  ["KHARKA", "Integrations", "kharka", "Binary", "Path to kharka (empty = from PATH)."],
  ["KHARKA_WATCH_STALE", "Integrations", "kharka", "Restart watch after", "Restart herdr-kharka when its entry has not been written for this long.", "s"],

  ["HOST_LABEL", "Machines & sync", "This machine", "Name of this machine", "How this machine appears in agent lists."],
  ["MACHINE_LABELS", "Machines & sync", "This machine", "Machine aliases", "name=Alias pairs, e.g. omarchy-flo=Omarchy: what the panels call a machine (Servers, agents, topics)."],
  ["MACHINES_OFF", "Machines & sync", "This machine", "Hidden machines", "Comma-separated machines left out of the agent list."],
  ["STORE", "Machines & sync", "Storage", "Topic storage", "Where topics live: local, a replica on the store machine, or kharka."],
  ["STORE_MACHINE", "Machines & sync", "Storage", "Store machine", "The saved herdr machine that holds the replica."],
  ["SYNC_VIA", "Machines & sync", "Storage", "Sync through", "How replicas travel: herdr or plain ssh."],
  ["REMOTE_AGENTS", "Machines & sync", "Remote agents", "Show agents of other machines", "Also list the agents of the other machines."],
  ["AGENTS_VIA", "Machines & sync", "Remote agents", "Fetch remote agents through", "herdr (over its ssh connection) or kharka.", ""],
  ["PUSH_STATES", "Machines & sync", "Remote agents", "Push agent states", "Tell other machines at once when an agent changes state (herdr only).", ""],
  ["REMOTE_EVERY", "Machines & sync", "Remote agents", "Check other machines every", "How often the other machines are queried.", "s"],



];
// What a setting needs. dep: all of these must hold, else the row is dimmed (a sub-setting). needs: at least one of these must hold, else the row
// warns (a feature without a source). "KEY" holds when that switch is on (or a secret is set), "KEY=a|b" when its value is one of those.
const SETTING_DEPS = {
  DEPLOY_EVERY: { dep: ["DEPLOY_MONITOR"] }, DEPLOY_PROJECTS_OFF: { dep: ["DEPLOY_MONITOR"] },
  DEPLOY_MONITOR: { needs: ["DEPLOY_GITHUB", "DEPLOY_CIRCLECI"] },
  DEPLOY_GITHUB: { dep: ["DEPLOY_MONITOR"] }, DEPLOY_CIRCLECI: { dep: ["DEPLOY_MONITOR"], needs: ["CIRCLECI_TOKEN"] },
  DEPLOY_PIPELINES: { dep: ["DEPLOY_MONITOR", "DEPLOY_CIRCLECI"] },
  TEST_EVERY: { dep: ["TEST_MONITOR"] }, TEST_PROJECTS_OFF: { dep: ["TEST_MONITOR"] },
  TEST_MONITOR: { needs: ["TEST_LOCAL", "TEST_GITHUB", "TEST_CIRCLECI"] },
  TEST_LOCAL: { dep: ["TEST_MONITOR"] }, TEST_STATUS_DIR: { dep: ["TEST_MONITOR", "TEST_LOCAL"] },
  TEST_GITHUB: { dep: ["TEST_MONITOR"] }, TEST_CIRCLECI: { dep: ["TEST_MONITOR"], needs: ["CIRCLECI_TOKEN"] },
  SLACK_EVERY: { dep: ["SLACK_WATCH"] }, CLAUDE_CONFIG_DIR: { dep: ["SLACK_WATCH"] }, FRIENDLY_REMINDER: { dep: ["CLAUDE_MOD"] },
  AUTO_ASSIGN_MACHINE: { dep: ["AUTO_ASSIGN"] },
  STORE_MACHINE: { dep: ["STORE=remote"] }, SYNC_VIA: { dep: ["STORE=remote"] },
  KHARKA: { needs: ["STORE=kharka", "AGENTS_VIA=kharka"] }, KHARKA_WATCH_STALE: { needs: ["STORE=kharka", "AGENTS_VIA=kharka"] },
  AGENTS_VIA: { dep: ["REMOTE_AGENTS"] }, PUSH_STATES: { dep: ["REMOTE_AGENTS", "AGENTS_VIA=herdr"] },
  REMOTE_EVERY: { dep: ["REMOTE_AGENTS"] }, MACHINES_OFF: { dep: ["REMOTE_AGENTS"] },
};
const SETTING_INFO = new Map(SETTINGS.map((r, i) => [r[0], { order: i, group: r[1], sec: r[2], name: r[3], desc: r[4], unit: r[5] ?? "", dep: SETTING_DEPS[r[0]]?.dep ?? [], needs: SETTING_DEPS[r[0]]?.needs ?? [] }]));
const SETTING_GROUPS = [...new Set(SETTINGS.map((r) => r[1]))];
const GROUP_ORDER = ["Topics", "Agents", "Monitors", "Runbooks", "Servers", "Integrations", "Machines & sync"];
function settingLabel(key) { const w = key.toLowerCase().replace(/_/g, " "); return w[0].toUpperCase() + w.slice(1); }

export default class Bhote extends View {
  init(_props, cx) {
    if (props()?.settings) { this.initSettings(cx); return; }   // the settings dialog
    if (props()?.headless) { this.initHeadless(cx); return; }   // the form for another headless agent
    if (props()?.menu) { this.menu = props().menu; return; }   // the Options popup: a list of items, nothing to load
    this.side = props()?.side === "left" ? "left" : "right";
    // the tab that is open (the overview of four tiles, or the tab of a tile): kept in the shared state, so a reload does not throw it away
    try { this.tab = ["home", "topics", "prs", "deploys", "tests", "servers"].includes(shared("bhote.tab")) ? shared("bhote.tab") : "home"; } catch (_e) { this.tab = "home"; }
    // How bhote names this machine and the others (HOST_LABEL, MACHINE_LABELS in its config): a topic is started on an agent by those names.
    this.hostLabel = window()?.platform === "macos" ? "Mac" : "local";
    this.labels = "";
    this.deployOff = new Set();   // DEPLOY_PROJECTS_OFF: projects whose deployments are not shown (not deployed through CI, or not wanted)
    this.testsOff = new Set();    // TEST_PROJECTS_OFF
    const list = (v) => new Set(v.split(",").map((x) => x.trim()).filter(Boolean));
    this.cfgPromise = Promise.resolve(readTextFile(`${home()}/.config/bhote/config`)).then((text) => {
      for (const l of (text ?? "").split("\n")) {
        if (l.startsWith("MACHINE_LABELS=")) this.labels = l.slice(15).trim();
        else if (l.startsWith("HOST_LABEL=")) this.hostLabel = l.slice(11).trim() || this.hostLabel;
        else if (l.startsWith("DEPLOY_PROJECTS_OFF=")) this.deployOff = list(l.slice(20));
        else if (l.startsWith("TEST_PROJECTS_OFF=")) this.testsOff = list(l.slice(18));
        else if (l.startsWith("KHARKA=")) this.kharkaBin = l.slice(7).trim();
        else if (l.startsWith("AGENT_LISTEN=")) this.listenDefault = l.slice(13).trim() !== "off";
        else if (l.startsWith("FINISHED_MINUTES=")) { const n = Number(l.slice(17).trim()); if (Number.isFinite(n)) this.finMin = n; }
      }
    }).catch(() => { /* the defaults stay */ });
    this.svcList = [];            // the services (servers.json)
    this.svcState = {};           // service id: "up" | "booting" | "down", from the last probe
    this.svcFail = {};            // service id: { at, reason, lines }: a start that did not work
    this.kharkaBin = "";          // KHARKA: the kharka binary (empty: from the PATH)
    this.listenDefault = true;    // AGENT_LISTEN: do agents accept messages from other agents unless their card says otherwise
    this.listenMap = {};          // `${node}/${name}`: the records of listen/ in kharka ({ listening, name, at })
    this.msgDone = new Set();     // messages (msg/… in kharka) that are handed over or not for us
    this.msgWaiting = {};         // `${node}/${name}`: how many messages wait for that agent
    this.finMin = 30;             // FINISHED_MINUTES: how long an agent counts as recently finished (0: until it works again)
    this.prevSt = {};             // an agent's address: its status at the last look
    this.topics = [];
    this.tests = [];
    this.localTests = [];
    this.remoteTests = {};   // by machine: the test runs its bhote collector lists (a run on Omarchy is not in this computer's list)
    this.remoteAt = 0;
    this.remoteLoading = false;
    this.liveTests = {};     // by machine: test processes nobody reported (LIVE_TESTS_SH), as rows of the test list
    this.liveAt = 0;
    this.deploys = [];
    this.error = null;
    this.loading = false;
    this.projects = [];
    this.prs = [];
    this.builds = [];
    this.prLoading = false;
    this.prAt = 0;
    // the show view: which topic it showed last, edit mode with its fields, the delete confirmation
    this.shownId = null;
    this.showArchived = false;
    this.editing = false;
    this.confirmDelete = false;
    if (this.side === "right") {
      this.adding = false;
      this.addIn = InputState.new({ placeholder: "New topic: Title; description", value: "" });
      // (its own placeholder is drawn over the field, italic and muted: the state's cannot be styled)
      this.stashIn = TextareaState.new({ placeholder: "", value: "", rows: 1 });
      this.stashIn.set_auto_grow(1, 4);
      this.stashIn.on("submit", (ev, cx) => { if (ev?.secondary) this.addStash(cx); });   // Return is a new line, the Send button (or Cmd+Return) posts
      this.stashIn.on("change", (_ev, cx) => cx.notify());
      this.drag = null;        // a stash being moved: { id, from, to }
      this.dragMoved = false;  // (a press that moved a stash is not a click that edits it)
      this.editingStash = null;
      this.stashEdit = TextareaState.new({ placeholder: "", value: "", rows: 1 });
      this.stashEdit.set_auto_grow(1, 5);
      this.stashEdit.on("submit", (ev, cx) => { if (ev?.secondary) this.saveStash(cx); });
      this.addIn.on("submit", (_e, cx) => this.addTopic(cx));
      this.titleIn = InputState.new({ value: "" });
      this.titleIn.on("submit", (_e, cx) => this.saveEdit(cx));
      this.descIn = TextareaState.new({ value: "", rows: 4 });
      this.descIn.on("change", (_e, cx) => cx.notify());
    }
    // How long runs took (seconds, the last five of each project and workflow), kept for the time left of the next one.
    try { this.hist = JSON.parse(localStorage.getItem("hist") ?? "{}"); } catch (_e) { this.hist = {}; }
    this.track = {};      // runs seen running: id -> true (a finish seen later teaches its duration)
    this.learned = new Set();
    cx.timer.every(5000, (cx) => this.load(cx));
    this.load(cx);
    if (this.side === "right") { cx.timer.every(90000, (cx) => this.loadPRs(cx)); }
    // the servers: the IMS systems of every machine; polled while the overview or the Servers tab is open
    this.srv = {};          // by machine: what servers.js reported
    this.srvInfo = {};      // by machine: where its IMS checkout and its node are
    this.srvPending = {};   // `${machine}|${slot}` (or "tunnel"): what was asked for, until the poll shows it
    this.srvLoading = false;
    this.srvAt = 0;
    this.srvHover = null;
    if (this.side === "right") cx.timer.every(3000, (cx) => this.tickServers(cx));
    // the version in the branding of the foot row: the last one seen at once, then what `bhote --version` says now
    try { this.version = localStorage.getItem("version") ?? ""; } catch (_e) { this.version = ""; }
    if (this.side === "right") cx.spawn(async (cx) => {
      try {
        const v = (await exec("bhote", ["--version"], { timeout: 15000 })).stdout.trim().replace(/^bhote\s+/i, "");
        if (v && v !== this.version) { this.version = v; try { localStorage.setItem("version", v); } catch (_e) { /* not kept */ } cx.notify(); }
      } catch (_e) { /* the last one stays */ }
    });
    if (this.side === "right") cx.timer.after(3000, (cx) => { const m = this.cfgMeta(); if (!m || Date.now() - (m.at ?? 0) > 12 * 3600 * 1000) this.fetchCfgMeta(cx, null); });   // (the settings dialog opens at once)
    if (this.side === "right") cx.timer.every(4000, () => this.publishDeploys());
    if (this.side === "right") { this.loadLiveTests(cx); cx.timer.every(5000, (cx) => this.loadLiveTests(cx)); }
    this.pulsing = false;
    if (this.side === "right") cx.timer.every(100, (cx) => { if (this.pulsing) { this.pulsing = false; cx.notify(); } });   // (a render that draws a pulsing circle sets it again)
    // the scheduled headless agents (timers that run Claude: slack monitors, depbot ...) of every machine, in the left panel
    this.jobs = {};         // by machine: [{ timer, svc, name, active, next, last, running, failed }]
    this.jobPending = {};   // `${machine}|${timer}`: "run" | "pause" | "resume" until the poll shows it
    this.commits = {};      // Commit-Inspector, by `${machine}|${folder}`: { at, list: [{ hash, subject, time, author }], dirty, error }
    this.commitLoading = false;
    this.jobAt = 0;
    this.jobLoading = false;
    this.mons = {};         // by machine: the sessions with a monitor (a recurring job in a Claude Code session): [{ cwd, jobs: [...] }]
    this.agentCwd = {};     // an agent's address: the folder it runs in
    this.monReady = {};     // machines where monitors.py is in place
    this.monLoading = false;
    if (this.side === "left") { this.trackFinished(); this.trackSpaces(); this.tick = 0; cx.timer.every(70, (cx) => { this.tick += 1; if (this.tick % 7 === 0 && this.trackFinished()) cx.notify(); else if (this.animating || Date.now() < (this.commitAnimUntil ?? 0)) cx.notify(); }); cx.timer.every(2000, (cx) => { this.trackSpaces(); try { const v = shared("bhote.jobsRefresh"); if (v && v !== this.jobsSeen) { this.jobsSeen = v; this.jobLoading = false; this.loadJobs(cx); } } catch (_e) { /* the 15 s poll covers it */ } }); }
    if (this.side === "left") { this.loadJobs(cx); this.loadMonitors(cx); cx.timer.every(15000, (cx) => { this.loadJobs(cx); this.loadMonitors(cx); }); }
    if (this.side === "left") { this.loadListen(cx); cx.timer.every(5000, (cx) => this.loadListen(cx)); cx.timer.every(4000, (cx) => this.loadMessages(cx)); }
    if (this.side === "left") { this.loadCommits(cx); cx.timer.every(1000, (cx) => this.loadCommits(cx)); }
    // Topics are plain files: reload when one changes.
    cx.spawn(async (cx) => {
      const dir = `${home()}/.local/share/bhote/topics`;
      try { for (;;) { await watchFolder(dir); this.load(cx); } } catch (_e) { /* no folder yet: the timer covers it */ }
    });
  }

  // One sh call reads every topic file and the two monitor lists (`bhote list --json` is bash and jq: ~14 s for 140 topics).
  load(cx) {
    if (this.loading) return;
    this.loading = true;
    cx.spawn(async (cx) => {
      try {
        const script =
          'd="${XDG_DATA_HOME:-$HOME/.local/share}/bhote/topics"; for f in "$d"/*.topic; do [ -f "$f" ] && { printf "\\036\\n"; cat "$f"; echo; }; done; ' +
          SHARED_SH +
          'printf "\\037T\\n"; cat "$s/tests.list" 2>/dev/null; printf "\\037D\\n"; cat "$s/deploys.list" 2>/dev/null';
        const r = await exec("sh", ["-c", script], { timeout: 15000 });
        const [topics, rest = ""] = r.stdout.split("\x1fT\n");
        const [tests = "", deploys = ""] = rest.split("\x1fD\n");
        this.topics = parseTopics(topics);
        this.prunePending();
        this.runQueue();
        this.localTests = parseMonitor(tests);
        this.tests = this.mergeTests();
        this.deploys = parseMonitor(deploys);
        this.projects = parseProjects(topics);
        this.ringMap = parseRing(topics);
        this.observeRows(this.tests, "t");
        this.observeRows(this.deploys, "d");
        if (this.side === "right") this.loadRemoteTests(cx);
        if (this.side === "right" && !this.prAt) this.loadPRs(cx);
        this.error = null;
      } catch (e) {
        this.error = String(e?.message ?? e);
      }
      this.loading = false;
      cx.notify();
    });
  }

  // This computer's test runs and those of the other machines, newest first. A run of a worktree on another machine (an agent in IMS-wt1
  // on Omarchy) is only in that machine's list: asked for every 8 s, with the machine's name on each row.
  mergeTests() {
    const rows = [...this.localTests];
    for (const [m, list] of Object.entries(this.remoteTests)) for (const r of list) rows.push({ ...r, machine: m });
    // a run seen as a process is only added when no reported run of that project is running already (a status file says the same, better)
    for (const [m, list] of Object.entries(this.liveTests)) for (const r of list) {
      const same = rows.some((x) => x.state === "running" && !x.live && this.projectOf(x.project) === this.projectOf(r.project) && (x.machine ?? "local") === (m === "local" ? "local" : m));
      if (!same) rows.push({ ...r, machine: m === "local" ? undefined : m });
    }
    rows.sort((a, b) => b.time - a.time);
    // a failed run that a later passing run of the same suite (same project and label, same branch when both say it) has fixed is "fixed":
    // it is shown as failed-fixed and no longer counted as failed
    for (let i = 0; i < rows.length; i++) {
      const x = rows[i];
      if (x.state !== "failed") continue;
      const later = rows.slice(0, i).some((y) => y.state === "ok" && y.project === x.project && y.label === x.label && (!x.branch || !y.branch || x.branch === y.branch) && y.time > x.time);
      if (later) rows[i] = { ...x, state: "fixed" };
    }
    return rows.slice(0, 60);
  }

  // The test processes of every machine, every 5 s (the right panel). A run is a row for as long as its process lives.
  loadLiveTests(cx) {
    if (this.liveLoading || Date.now() - this.liveAt < 4000) return;
    this.liveLoading = true; this.liveAt = Date.now();
    cx.spawn(async (cx) => {
      let changed = false;
      for (const m of this.srvMachines()) {
        const key = m.name == null ? "local" : m.label;
        try {
          const r = await this.srvExec(m.name, "sh", ["-c", LIVE_TESTS_SH], 15000);
          const now = Math.floor(Date.now() / 1000);
          const rows = r.stdout.split("\n").filter(Boolean).map((l) => {
            const [pid, secs, top, branch, ...cmd] = l.split("\t");
            const c = cmd.join(" ");
            const runner = /(rails test|rake test|rspec|vitest|jest|pytest|mocha|(?:yarn|npm|pnpm)(?: run)? test)/.exec(c)?.[1] ?? "tests";
            const args = c.slice(c.indexOf(runner) + runner.length).trim();
            return { time: now - (Number(secs) || 0), project: top.split("/").pop(), label: runner, state: "running", done: 0, total: 0, branch: branch ?? "",
              title: args.length > 90 ? `${args.slice(0, 89)}…` : args, url: "", live: true, pid };
          });
          if (JSON.stringify((this.liveTests[key] ?? []).map((x) => x.pid)) !== JSON.stringify(rows.map((x) => x.pid))) changed = true;
          this.liveTests[key] = rows;
        } catch (_e) { /* that machine does not answer: its last rows stay */ }
      }
      this.liveLoading = false;
      if (changed) { this.tests = this.mergeTests(); cx.notify(); }
    });
  }

  loadRemoteTests(cx) {
    const names = this.srvMachines().filter((m) => m.name != null);
    if (!names.length || this.remoteLoading || Date.now() - this.remoteAt < 8000) return;
    this.remoteLoading = true;
    this.remoteAt = Date.now();
    const script = SHARED_SH + 'cat "$s/tests.list" 2>/dev/null';
    cx.spawn(async (cx) => {
      let changed = false;
      for (const m of names) {
        try {
          const r = await exec("hc", ["exec", "--on", m.name, "sh", "-c", script], { timeout: 15000 });
          const rows = parseMonitor(r.stdout);
          this.observeRows(rows, "t");
          this.remoteTests[m.label] = rows;
          changed = true;
        } catch (_e) { /* that machine does not answer: its last list stays */ }
      }
      if (changed) this.tests = this.mergeTests();
      this.remoteLoading = false;
      cx.notify();
    });
  }

  // --- how long runs take, and the time left of the running ones ------------------------------------------------------
  learn(key, sec) {
    if (!(sec > 5 && sec < 6 * 3600)) return;
    const l = [...(this.hist[key] ?? []), Math.round(sec)].slice(-5);
    this.hist[key] = l;
    try { localStorage.setItem("hist", JSON.stringify(this.hist)); } catch (_e) { /* not remembered */ }
  }

  typical(key) { return median(this.hist[key] ?? []); }

  // A run of a monitor list: seen running, then finished: its duration is what it took.
  observeRows(rows, kind) {
    const now = Date.now() / 1000;
    for (const x of rows) {
      const key = `${kind}|${x.project}|${x.label}`, id = `${key}|${x.time}`;
      if (x.state === "running") this.track[id] = true;
      else if (x.state === "ok" && this.track[id] && !this.learned.has(id)) { this.learn(key, now - x.time); this.learned.add(id); delete this.track[id]; }
    }
  }

  // The checks of a pull request: a check run says when it started and ended; a status (CircleCI) only when it started.
  observeChecks(project, checks) {
    const now = Date.now() / 1000;
    for (const c of checks) {
      c.key = `c|${project}|${c.name}`;
      const id = `${c.key}|${c.started}`;
      if (c.state === "running") this.track[id] = true;
      else if (c.state === "ok" && c.started && !this.learned.has(id) && (c.ended || this.track[id])) { this.learn(c.key, (c.ended || now) - c.started); this.learned.add(id); delete this.track[id]; }
    }
  }

  // Progress (0..1 or null) and the seconds left (or null) of a running monitor row.
  runEstimate(x, kind) {
    if (x.est) return x.est;
    const elapsed = Date.now() / 1000 - x.time;
    const med = this.typical(x.histKey ?? `${kind}|${x.project}|${x.label}`);
    const byStep = x.total > 0 ? x.done / x.total : null;
    const frac = byStep ?? (med ? Math.min(0.95, elapsed / med) : null);
    const remain = med ? Math.max(0, med - elapsed) : byStep && x.done > 0 ? (elapsed * (x.total - x.done)) / x.done : null;
    return { frac, remain };
  }

  // A pull request's checks: the share that finished, and the longest time left of those running.
  checkEstimate(checks) {
    const total = checks.length;
    const finished = checks.filter((c) => c.state !== "running").length;
    const now = Date.now() / 1000;
    let remain = null;
    for (const c of checks) {
      if (c.state !== "running") continue;
      const med = this.typical(c.key);
      if (med && c.started) remain = Math.max(remain ?? 0, Math.max(0, med - (now - c.started)));
    }
    return { frac: total ? finished / total : null, finished, total, remain };
  }

  // --- changes shown at once -----------------------------------------------------------------------------------------------------
  // `bhote` is a bash script and takes seconds. A change is shown the moment it is made: it is noted as pending in the shared state (so
  // that the menu popup can note it too), the lists read the topics with the pending changes applied, and a change is dropped when the
  // topic files have caught up (or after 45 s, when the command did not do it).
  get topics() { return this.withPending(this._topics ?? []); }
  set topics(v) { this._topics = v; }

  pendingNow() {
    const n = Date.now();
    if (this._pendAt && n - this._pendAt < 40) return this._pend;
    let p = {};
    try { p = shared("bhote.pending") ?? {}; } catch (_e) { p = {}; }
    this._pend = p; this._pendAt = n;
    return p;
  }

  markPending(ids, patch) {
    let p = {};
    try { p = { ...(shared("bhote.pending") ?? {}) }; } catch (_e) { p = {}; }
    const at = Date.now();
    for (const id of ids) p[id] = { ...patch, at };
    setShared("bhote.pending", p);
    this._pendAt = 0;
  }

  withPending(list) {
    const p = this.pendingNow();
    const keys = Object.keys(p);
    if (!keys.length) return list;
    const now = Math.floor(Date.now() / 1000);
    const out = list.filter((x) => !p[x.id]?.deleted).map((x) => {
      const e = p[x.id];
      if (!e || !e.status || (x.status === e.status && !x.dismissed)) return x;
      return { ...x, status: e.status, group: e.status, dismissed: false, updated: now };
    });
    for (const k of keys) {   // a topic that was just added: shown until its file is there
      const c = p[k].create;
      if (c && !list.some((x) => x.title === c.title && x.created * 1000 >= p[k].at - 5000)) {
        out.push({ id: k, title: c.title, desc: c.desc ?? "", status: "next", group: "next", waiting_for: null, updated: now, created: now, since: 0, dismissed: false, agent: null, machine: "", branch: "", project: "" });
      }
    }
    return out;
  }

  prunePending() {
    const p = this.pendingNow();
    const keys = Object.keys(p);
    if (!keys.length) return;
    const raw = this._topics ?? [];
    const now = Date.now();
    const next = {};
    let changed = false;
    for (const k of keys) {
      const e = p[k];
      const real = raw.find((x) => x.id === k);
      const caught = e.create ? raw.some((x) => x.title === e.create.title && x.created * 1000 >= e.at - 5000)
        : e.deleted ? !real : !!real && real.status === e.status && !real.dismissed;
      if (caught || now - e.at > 45000) { changed = true; continue; }
      next[k] = e;
    }
    if (changed) { setShared("bhote.pending", next); this._pendAt = 0; }
  }

  cli(cx, args) {
    const STATUS = new Set(["now", "review", "next", "later", "done", "testing"]);
    if (STATUS.has(args[0]) && args[1]) this.markPending([args[1]], { status: args[0] });
    else if (args[0] === "rm" && args[1]) this.markPending([args[1]], { deleted: true });
    cx.notify();
    cx.spawn(async (cx) => {
      try { await exec("bhote", args, { timeout: 30000 }); } catch (_e) { /* shown by the next load */ }
      this.load(cx);
    });
  }

  open(url) {
    if (url) exec("open", [url]).catch(() => {});
  }

  // --- the show view: one topic in the right panel -----------------------------------------------------------------------
  showId() {
    try { return shared("bhote.show") ?? null; } catch (_e) { return null; }
  }

  // Opens a topic in the right panel (from a card, a line or a done topic) and makes sure that panel is there.
  openPage(id) {
    setShared("bhote.show", id);
    run("layout.show", { id: "panel" });
    run("layout.select", { stack: "panel-body", child: "bhote-right" });
  }

  closePage() {
    setShared("bhote.show", null);
  }

  statusColor(t, status) {
    return { now: t.working, testing: t.ansi6, review: t.ansi4, finished: t.done, done: t.done, next: t.ansi3, waiting: t.ansi3, deploying: t.ansi3, later: t.dim }[status] ?? t.dim;
  }

  // A button of the show view.
  btn(t, id, label, onClick, o = {}) {
    return div().id(id).px(10).py(5).rounded(6).text_size(12).cursor_pointer()
      .bg(o.primary ? t.solid : t.raised).border(1).border_color(o.primary ? t.solid : o.danger ? "#a8162e" : t.border)
      .text_color(o.primary ? t.solid_foreground : o.danger ? t.blocked : t.foreground)
      .hover((x) => x.opacity(0.88)).on_click(onClick).child(label);
  }

  // A block of the show view: a small heading over a card.
  block(t, title, ...children) {
    return v_flex().mx(10).mb(10).gap(6)
      .child(div().px(2).text_size(11).font_semibold().text_color(t.dim).child(title.toUpperCase()))
      .children(children);
  }

  fact(t, label, value, color) {
    return h_flex().gap(10).text_size(12)
      .child(div().w(84).flex_none().text_color(t.dim).child(label))
      .child(div().flex_1().min_w(0).text_color(color ?? t.foreground).child(value));
  }

  startEdit(x, field) {
    this.titleIn.set_value(x.title);
    this.descIn.set_value(x.desc);
    this.editing = true;
    (field === "desc" ? this.descIn : this.titleIn).focus();
  }

  // Title and description through the CLI (rename, desc), then the lists reload.
  saveEdit(cx) {
    const x = this.topics.find((y) => y.id === this.showId());
    if (!x) return;
    const title = this.titleIn.value().trim();
    const desc = this.descIn.value().trim().slice(0, 140);
    this.editing = false;
    cx.notify();
    cx.spawn(async (cx) => {
      try {
        if (title && title !== x.title) await exec("bhote", ["rename", x.id, title], { timeout: 30000 });
        if (desc !== x.desc) await exec("bhote", desc ? ["desc", x.id, desc] : ["desc", x.id], { timeout: 30000 });
      } catch (_e) { /* the next load shows what is there */ }
      this.load(cx);
    });
  }

  // The show view: a bar to go back, then the topic as blocks: what it is and how it moves, who works on it, its branch
  // and pull request, the tests and deployments that belong to it, its timeline.
  showView() {
    const t = theme();
    const id = this.showId();
    if (id !== this.shownId) { this.shownId = id; this.editing = false; this.confirmDelete = false; }
    const x = this.topics.find((y) => y.id === id);
    const bar = h_flex().flex_none().px(8).pt(8).pb(6).gap(8).items_center().border_b(1).border_color(t.border)
      .child(div().id("show-back").w(30).h(28).flex_none().flex().items_center().justify_center().rounded(6).text_size(15).cursor_pointer()
        .bg(t.raised).border(1).border_color(t.border).text_color(t.foreground)
        .hover((e) => e.bg(t.hover)).on_click(() => this.closePage()).child("‹"))
      .child(div().flex_1().truncate().text_size(11).text_color(t.dim).child(x ? x.id : ""))
      .when(!!x && !this.editing, (el) => el.child(this.btn(t, "show-edit", "Edit", (_e, cx) => { this.startEdit(x); cx.notify(); })));
    if (!x) return v_flex().size_full().bg(t.chrome).child(bar).child(this.loading ? empty(t, "", "Loading", "", true) : empty(t, "∅", "This topic is gone", "It was deleted or merged into another one."));

    const sp = spaces();
    const machineOf = (a) => sp.find((q) => q.id === a.space)?.machine ?? null;
    const agent = this.agentOf(x, agents(), machineOf);
    const st = this.cardState(t, x, agent);
    const when = (sec) => (sec ? `${clock(sec)} · ${ago(sec)}` : "");
    const project = this.projects.find((p) => p.id === x.project);
    const body = [];

    // 1. the topic: title, state, description (or the fields while editing)
    if (this.editing) {
      body.push(this.block(t, "Edit",
        v_flex().gap(8).p(10).rounded(8).bg(t.surface).border(1).border_color(t.border)
          .child(div().text_size(11).text_color(t.dim).child("Title"))
          .child(Input.new(this.titleIn).h(34).px(10).rounded(7).border_1().border_color(t.accent).bg(t.background).text_size(13).text_color(t.foreground))
          .child(h_flex().child(div().flex_1().text_size(11).text_color(t.dim).child("Description"))
            .child(div().text_size(11).text_color(this.descIn.value().length > 140 ? t.blocked : t.dim).child(`${this.descIn.value().length}/140`)))
          .child(Textarea.new(this.descIn).h(86).px(10).py(6).rounded(7).border_1().border_color(t.input).bg(t.background).text_size(12.5).text_color(t.foreground))
          .child(h_flex().gap(8).justify_end()
            .child(this.btn(t, "edit-cancel", "Cancel", (_e, cx) => { this.editing = false; cx.notify(); }))
            .child(this.btn(t, "edit-save", "Save", (_e, cx) => this.saveEdit(cx), { primary: true })))));
    } else {
      body.push(h_flex().mx(10).mt(10).mb(10).bg(t.surface).border(1).border_color(t.border).rounded(8)
        .child(div().w(3).flex_none().rounded(2).my(8).ml(8).bg(st.bar))
        .child(v_flex().flex_1().min_w(0).gap(8).p(12)
          .child(div().text_size(17).font_semibold().text_color(t.foreground).child(x.title || "(untitled)"))
          .child(h_flex().gap(8).items_center().flex_wrap().text_size(11)
            .child(div().px(8).py(2).rounded(10).border(1).border_color(st.bar).text_color(st.color).child(st.word))
            .when(!!x.waiting_for, (el) => el.child(div().text_color(t.ansi3).child(`waiting for ${x.waiting_for}`)))
            .when(x.updated > 0, (el) => el.child(div().text_color(t.dim).child(`updated ${ago(x.updated)}`))))
          .child(h_flex().gap(8).items_start()
            .child(x.desc ? div().flex_1().min_w(0).text_size(13).text_color(t.muted).child(x.desc) : div().flex_1().text_size(12).text_color(t.dim).child("No description yet"))
            .child(div().id("edit-desc").flex_none().px(7).py(3).rounded(5).text_size(12).cursor_pointer().text_color(t.dim)
              .hover((e) => e.bg(t.hover).text_color(t.foreground))
              .on_click((_e, cx) => { this.startEdit(x, "desc"); cx.notify(); }).child(x.desc ? "✎ Edit" : "＋ Add description")))));
    }

    // 2. move it: the statuses as a row of pills, the current one in its colour
    body.push(this.block(t, "Move to", h_flex().gap(6).flex_wrap().children(
      [["now", "Now"], ["review", "Review"], ["next", "Next"], ["later", "Later"], ["done", "Done"]].map(([k, label]) => {
        const on = x.status === k;
        const c = this.statusColor(t, k);
        return div().id(`mv-${k}`).px(11).py(5).rounded(14).text_size(12).cursor_pointer()
          .bg(on ? tint(c, "33", t.selected) : t.surface).border(1).border_color(on ? c : t.border).text_color(on ? c : t.muted)
          .hover((e) => e.bg(on ? tint(c, "40", t.selected) : t.raised))
          .on_click((_e, cx) => { if (!on) this.cli(cx, [k, x.id]); }).child(label);
      }))));

    // 3. start it on an agent of your choice
    if (this.startItems(x).length) body.push(this.block(t, "Start", h_flex().gap(8).items_center()
      .child(this.btn(t, "show-start", "Start on agent…", () => this.openMenu(this.startItems(x))))
      .child(div().flex_1().min_w(0).text_size(11).text_color(this.queueOf(x.id) ? t.ansi3 : t.dim)
        .child(this.queueOf(x.id) ? `queued on ${this.queueOf(x.id).name}, starts when it is free` : x.agent ? `last started on ${x.agent}` : "not started on an agent yet"))
      .when(!!this.queueOf(x.id), (el) => el.child(this.btn(t, "show-unqueue", "Cancel", () => setShared("bhote.queue", this.queueNow().filter((e) => e.id !== x.id)))))));

    // 3b. where it was started
    if (x.agent) {
      body.push(this.block(t, "Agent",
        v_flex().gap(6).p(10).rounded(8).bg(t.surface).border(1).border_color(t.border)
          .child(h_flex().gap(8).items_center()
            .child(this.agentIcon(t, agent))
            .child(div().flex_1().min_w(0).truncate().font_semibold().text_color(t.foreground).child(x.agent))
            .when(!isLocalName(x.machine), (el) => el.child(div().flex_none().text_size(11).text_color(t.ansi4).child(x.machine)))
            .child(div().flex_none().text_size(11).text_color(agent ? this.cardState(t, { ...x, group: "now" }, agent).color : t.dim).child(agent ? agent.status : "not here")))
          .when(!!agent && agent.status === "working" && !!agent.title, (el) => el.child(div().pl(22).text_size(11).text_color(t.working).child(agent.title)))
          .when(!!agent, (el) => el.child(h_flex().pl(22).child(this.btn(t, "show-agent", "Show agent", () => run("agent.focus", { terminal: agent.address })))))));
    }

    // 4. its branch and pull request
    const prNum = /\bPR #(\d+)\b/.exec(x.title)?.[1];
    if (x.branch || prNum) {
      const inProject = (q) => !project || q.project === project.title;
      const pr = (x.branch && (this.prs.find((q) => q.branch === x.branch && !q.merged) ?? this.prs.find((q) => q.branch === x.branch)))
        || (prNum && this.prs.find((q) => String(q.number) === prNum && inProject(q))) || null;
      body.push(this.block(t, "Branch and pull request",
        pr ? this.prCard(t, "show-pr", pr, { noTopic: true }).mx(0)
          : v_flex().gap(4).p(10).rounded(8).bg(t.surface).border(1).border_color(t.border)
            .when(!!x.branch, (el) => el.child(div().text_size(12).text_color(t.foreground).child(`⎇ ${x.branch}`)))
            .child(div().text_size(11).text_color(t.dim).child(this.prLoading || !this.prAt ? "Asking GitHub..." : "No pull request found for it"))));
    }

    // 5. the tests of its branch and the deployments of its project
    const tests = this.tests.filter((r) => x.branch && r.branch === x.branch).slice(0, 3);
    if (tests.length) body.push(this.block(t, "Tests on this branch", ...tests.map((r, i) => this.monitorCard(t, `show-t-${i}`, r, "t").mx(0))));
    const deploys = project ? this.deploysAll().filter((r) => r.project === project.title).slice(0, 3) : [];
    if (deploys.length) body.push(this.block(t, `Deployments · ${project.title}`, ...deploys.map((r, i) => this.monitorCard(t, `show-d-${i}`, r, "d").mx(0))));

    // 6. the timeline
    body.push(this.block(t, "Timeline",
      v_flex().gap(5).p(10).rounded(8).bg(t.surface).border(1).border_color(t.border)
        .when(x.created > 0, (el) => el.child(this.fact(t, "Created", when(x.created))))
        .when(x.updated > 0, (el) => el.child(this.fact(t, "Updated", when(x.updated))))
        .when(x.since > 0, (el) => el.child(this.fact(t, "Waiting since", when(x.since))))
        .when(!!project, (el) => el.child(this.fact(t, "Project", project.title)))));

    // 7. delete, in two steps
    body.push(h_flex().mx(10).mb(14).gap(8).items_center()
      .when(!this.confirmDelete, (el) => el.child(this.btn(t, "show-delete", "Delete topic", (_e, cx) => { this.confirmDelete = true; cx.notify(); }, { danger: true })))
      .when(this.confirmDelete, (el) => el.child(div().text_size(12).text_color(t.blocked).child("Delete for good?"))
        .child(this.btn(t, "show-delete-yes", "Yes, delete", (_e, cx) => { this.cli(cx, ["rm", x.id]); this.closePage(); }, { danger: true }))
        .child(this.btn(t, "show-delete-no", "Cancel", (_e, cx) => { this.confirmDelete = false; cx.notify(); }))));

    return v_flex().size_full().bg(t.chrome).text_size(13).child(bar).child(v_flex().flex_1().min_h(0).overflow_y_scrollbar().children(body));
  }

  // --- the multi-select of topics and test runs: the ticked ones, kept in the shared state -------------------------------------------
  sel() {
    try { const v = shared("bhote.sel"); return { topics: new Set(v?.topics ?? []), tests: new Set(v?.tests ?? []) }; } catch (_e) { return { topics: new Set(), tests: new Set() }; }
  }

  setSel(topics, tests) {
    setShared("bhote.sel", { topics: [...topics], tests: [...tests] });
  }

  toggleSel(kind, key) {
    const s = this.sel();
    const set = kind === "topics" ? s.topics : s.tests;
    if (set.has(key)) set.delete(key); else set.add(key);
    this.setSel(s.topics, s.tests);
  }

  // The checkbox at the front of a card: a click ticks it and never opens the card.
  check(t, id, kind, key, on) {
    return div().id(id).w(24).h(24).flex_none().flex().items_center().justify_center().rounded(5).cursor_pointer()
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .on_click(this.guard((_e, cx) => { this.toggleSel(kind, key); cx.notify(); })).child(checkbox(t, on));
  }

  // The bar above a list, always there: its checkbox ticks everything, or clears the selection when something is ticked; it says how
  // much is ticked, and its menu holds the batch actions (it works when something is ticked).
  batchBar(t, kind, allKeys, extra) {
    if (!allKeys.length && kind !== "topics") return null;
    const s = this.sel();
    const set = kind === "topics" ? s.topics : s.tests;
    const n = allKeys.filter((k) => set.has(k)).length;
    const all = n === allKeys.length;
    const toggleAll = () => {
      const next = n > 0 ? new Set() : new Set(allKeys);
      if (kind === "topics") this.setSel(next, s.tests); else this.setSel(s.topics, next);
    };
    return h_flex().id(`batch-${kind}`).flex_none().gap(8).pl(8).pr(12).py(7).items_center().bg(t.surface).border_b(1).border_color(t.border)
      .child(div().id(`batch-all-${kind}`).w(24).h(24).flex_none().flex().items_center().justify_center().rounded(5).cursor_pointer()
        .on_click(this.guard((_e, cx) => { toggleAll(); cx.notify(); })).child(checkbox(t, n > 0 && all, n > 0 && !all)))
      .child(div().flex_1().text_size(12).text_color(n > 0 ? t.foreground : t.muted).child(n > 0 ? `${n} selected` : "Select all"))
      .when(!!extra, (el) => el.child(extra))
      .when(kind === "topics", (el) => el.child(div().id("add-topic").flex_none().flex().items_center().gap(5).px(10).py(4).rounded(6).text_size(12).cursor_pointer()
        .bg(this.adding ? t.selected : t.raised).border(1).border_color(this.adding ? t.accent : t.border).text_color(t.foreground).hover((e) => e.bg(t.hover))
        .on_click(this.guard((_e, cx) => { this.adding = !this.adding; if (this.adding) this.addIn.focus(); cx.notify(); }))
        .child(svg("mods/bhote/plus.svg").size(12).text_color(t.muted)).child(div().child("Add topic"))))
      .child(div().id(`batch-menu-${kind}`).flex_none().flex().items_center().gap(5).px(10).py(4).rounded(6).text_size(12)
        .bg(t.raised).border(1).border_color(t.border).text_color(t.foreground).opacity(n > 0 ? 1 : 0.45).hover((e) => e.bg(n > 0 ? t.hover : t.raised))
        .on_mouse_down("left", (e) => { this.anchor = this.anchorOf(e); })
        .when(n > 0, (el) => el.cursor_pointer().on_click(this.guard(() => this.openMenu(this.batchItems(kind, allKeys), this.anchor))))
        .child(div().child("Actions")).child(svg("mods/bhote/chevron-down.svg").size(12).text_color(t.dim)));
  }

  // The row under the bar where a topic is typed: "Title" or "Title; description" (Enter or Add makes it, as the next topic).
  addRow(t) {
    return v_flex().flex_none().gap(8).px(12).py(9).bg(t.surface).border_b(1).border_color(t.border)
      .child(Input.new(this.addIn).h(34).px(10).rounded(7).border_1().border_color(t.accent).bg(t.background).text_size(13).text_color(t.foreground))
      .child(h_flex().gap(8).items_center()
        .child(div().flex_1().text_size(11).text_color(t.dim).child("Title; description (up to 140 characters)"))
        .child(this.btn(t, "add-cancel", "Cancel", (_e, cx) => { this.adding = false; this.addIn.set_value(""); cx.notify(); }))
        .child(this.btn(t, "add-go", "Add", (_e, cx) => this.addTopic(cx), { primary: true })));
  }

  addTopic(cx) {
    const text = this.addIn.value().trim();
    if (!text) return;
    const i = text.indexOf(";");
    const title = (i < 0 ? text : text.slice(0, i)).trim();
    const desc = i < 0 ? "" : text.slice(i + 1).trim().slice(0, 140);
    if (!title) return;
    this.adding = false;
    this.addIn.set_value("");
    this.markPending([`new:${Date.now()}`], { create: { title, desc } });
    cx.notify();
    cx.spawn(async (cx) => {
      try { await exec("bhote", desc ? ["add", title, "-d", desc, "-s", "next"] : ["add", title, "-s", "next"], { timeout: 30000 }); } catch (_e) { /* the next load shows what is there */ }
      this.load(cx);
    });
  }

  // The actions on everything ticked, as menu items.
  batchItems(kind, allKeys) {
    const s = this.sel();
    const common = [{ type: "bsel", kind, op: "all", keys: allKeys, label: "Select all" }, { type: "bsel", kind, op: "none", label: "Clear selection" }];
    if (kind === "topics") {
      const ids = [...s.topics];
      return [{ type: "head", label: `${ids.length} topic${ids.length === 1 ? "" : "s"} selected` },
        ...["now", "review", "next", "later", "done"].map((k) => ({ type: "bstatus", ids, status: k, label: `Move to ${k[0].toUpperCase()}${k.slice(1)}` })),
        { type: "bask", label: "Delete…", danger: true, items: [{ type: "head", label: `Delete ${ids.length} topic${ids.length === 1 ? "" : "s"}?` }, { type: "bdelete", ids, label: "Yes, delete", danger: true }, { type: "noop", label: "Cancel" }] },
        { type: "head", label: "Selection" }, ...common];
    }
    const keys = [...s.tests];
    return [{ type: "head", label: `${keys.length} test run${keys.length === 1 ? "" : "s"} selected` },
      { type: "bhide", mode: "archive", keys, label: "Archive" },
      { type: "bhide", mode: "restore", keys, label: "Restore from archive" },
      { type: "bhide", mode: "discard", keys, label: "Discard", danger: true },
      { type: "head", label: "Selection" }, ...common];
  }

  // --- test runs a person archived or discarded: kept in the shared state, so they stay away after a restart -------------------
  runKey(x) { return `${x.project}|${x.label}|${x.time}`; }

  hidden() {
    try { const h = shared("bhote.hidden"); return { archived: new Set(h?.archived ?? []), discarded: new Set(h?.discarded ?? []) }; } catch (_e) { return { archived: new Set(), discarded: new Set() }; }
  }

  // The test runs the Tests tab shows (not archived, not discarded) and the archived ones.
  // The checks that run on open pull requests, one run per pull request (its finished share is the progress), so that the Tests tab and tile
  // show them as running too.
  prTests() {
    const out = [];
    for (const pr of this.prs) {
      if (pr.kind !== "pr" || pr.merged) continue;
      // only the test checks of the pull request: its builds are deployments, all its checks are its own tile
      const tests = pr.checks.filter((c) => /test|spec|lint|e2e/i.test(c.name));
      const run = tests.filter((c) => c.state === "running");
      if (!run.length) continue;
      const est = this.checkEstimate(tests);
      out.push({ time: Math.min(...run.map((c) => c.started || Math.floor(Date.now() / 1000))), project: pr.project, label: `PR #${pr.number}`, state: "running",
        done: est.finished, total: tests.length, branch: pr.branch, title: pr.title, url: pr.url, est: { frac: est.frac, remain: est.remain } });
    }
    return out;
  }

  testsShown() {
    const h = this.hidden();
    const mine = [...this.prTests(), ...this.tests].filter((x) => x.live || !this.testsOff.has(this.projectOf(x.project)))   // (a run that is going on shows even for a project switched off for CI).sort((a, b) => b.time - a.time);
    return { shown: mine.filter((x) => !h.archived.has(this.runKey(x)) && !h.discarded.has(this.runKey(x))),
      archived: mine.filter((x) => h.archived.has(this.runKey(x)) && !h.discarded.has(this.runKey(x))) };
  }

  // The archive of the Tests tab: a bar that stays at the bottom of the panel, and above it, when open, the archived runs in a
  // drawer of their own (at most 45% of the panel, it scrolls).
  archiveDock(t) {
    const { archived } = this.testsShown();
    if (!archived.length) return [];
    const open = this.showArchived;
    const out = [];
    if (open) out.push(v_flex().flex_none().max_h("45%").overflow_y_scrollbar().pt(6).border_t(1).border_color(t.border).bg(t.chrome)
      .children(archived.slice(0, 30).map((x, i) => this.monitorCard(t, `ar-${i}`, x, "t", true))));
    out.push(h_flex().id("arch-toggle").flex_none().px(12).py(9).gap(8).items_center().cursor_pointer().border_t(1).border_color(t.border).bg(t.surface)
      .text_size(12).text_color(t.muted).hover((e) => e.bg(t.raised))
      .on_click((_e, cx) => { this.showArchived = !this.showArchived; cx.notify(); })
      .child(div().w(10).child(open ? "▾" : "▸"))
      .child(div().flex_1().child(`Archived · ${archived.length}`)));
    return out;
  }

  // --- the Options menu of a card: a button at the bottom right, a popup with what can be done with it -------------------------
  statusItems(x) {
    return [{ type: "head", label: "Move topic to" },
      ...["now", "review", "next", "later", "done"].map((k) => ({ type: "status", status: k, id: x.id, label: `${k[0].toUpperCase()}${k.slice(1)}${x.status === k ? "   ✓" : ""}` }))];
  }

  // --- the queue: a topic that waits on a busy agent, kept in the shared state --------------------------------------------------------
  // An entry: { id (the topic), machineName (herdr-app's name of the machine, null: this one), machine (bhote's name), pane (the terminal), name }.
  queueNow() {
    try { const q = shared("bhote.queue"); return Array.isArray(q) ? q : []; } catch (_e) { return []; }
  }

  queueOf(id) {
    return this.queueNow().find((e) => e.id === id) ?? null;
  }

  // The topic goes to that agent now: it is shown as started at once and bhote hands it over.
  startTopic(id, machine, pane, name) {
    this.markPending([id], { status: "now" });
    exec("bhote", ["__act", "pickstart", id, machine, pane, name], { timeout: 60000 }).catch(() => {});
  }

  // Looked at on every reload: a queued topic whose agent is free is started there (one topic at a time per agent), one that is done or gone
  // is dropped from the queue.
  runQueue() {
    const q = this.queueNow();
    if (!q.length) return;
    const sp = spaces();
    const live = agents();
    const taken = new Set();
    const keep = [];
    for (const e of q) {
      const topic = (this._topics ?? []).find((x) => x.id === e.id);
      if (!topic || topic.group === "done") continue;
      const a = live.find((l) => l.address.split("/").pop() === e.pane && (sp.find((z) => z.id === l.space)?.machine ?? null) === e.machineName);
      const key = `${e.machineName}|${e.pane}`;
      const cooling = (this.queueCool?.[key] ?? 0) > Date.now();
      if (a && (a.status === "idle" || a.status === "done") && !taken.has(key) && !cooling) {
        taken.add(key);
        this.queueCool = { ...(this.queueCool ?? {}), [key]: Date.now() + 25000 };   // it takes a moment until the agent shows as working
        this.startTopic(e.id, e.machine, e.pane, e.name);
        continue;
      }
      keep.push(e);
    }
    if (keep.length !== q.length) setShared("bhote.queue", keep);
  }

  // bhote's name of a machine (null: this one): MACHINE_LABELS maps herdr-app's name to it.
  machineLabel(name) {
    if (name == null) return this.hostLabel;
    const hit = this.labels.split(",").map((p) => p.split("=")).find(([k]) => k?.trim() === name);
    return hit?.[1]?.trim() || name;
  }

  // A topic and an agent belong together only when the person starts the topic on one: the free agents, to pick from.
  startItems(x) {
    if (x.group === "done") return [];
    const sp = spaces();
    const all = agents().map((a) => ({ a, m: sp.find((q) => q.id === a.space)?.machine ?? null }));
    if (!all.length) return [{ type: "head", label: "No agent running anywhere" }];
    const free = all.filter(({ a }) => a.status === "idle" || a.status === "done");
    const busy = all.filter(({ a }) => !(a.status === "idle" || a.status === "done"));
    const where = (m) => this.machineLabel(m ?? null);   // (the alias: Mac, Omarchy)
    return [
      { type: "head", label: free.length ? "Start on agent" : "No free agent" },
      // every agent of every machine is listed; only a free one can take the topic, the others show what they are doing
      ...free.map(({ a, m }) => ({ type: "pickstart", id: x.id, machine: this.machineLabel(m), pane: a.address.split("/").pop(), name: a.space_name, label: `${a.space_name}  ·  ${where(m)}` })),
      ...(busy.length ? [{ type: "head", label: "Busy: queue it, it starts when the agent is free" }] : []),
      ...busy.map(({ a, m }) => ({ type: "queue", id: x.id, machineName: m, machine: this.machineLabel(m), pane: a.address.split("/").pop(), name: a.space_name,
        label: `${a.space_name}  ·  ${where(m)}  ·  ${a.status === "blocked" ? "needs you" : a.status}` })),
    ];
  }

  topicItems(x) {
    const q = this.queueOf(x.id);
    return [{ type: "show", id: x.id, label: "Show topic" }, ...(q ? [{ type: "unqueue", id: x.id, label: `Cancel the queue on ${q.name}` }] : []),
      ...this.statusItems(x), ...this.startItems(x)];
  }

  // The ... button of a card: it opens the menu of that card and never counts as a click on the card behind it.
  optionsBtn(t, id, getItems) {
    return div().id(id).w(26).h(22).flex_none().flex().items_center().justify_center().rounded(5).cursor_pointer().text_color(t.muted)
      .hover((e) => e.bg(t.hover).text_color(t.foreground))
      .on_mouse_down("left", (e, cx) => { this.anchor = this.anchorOf(e); cx.stop_propagation(); })
      .on_click(this.guard(() => this.openMenu(getItems(), this.anchor))).child(svg("mods/bhote/dots.svg").size(16));
  }

  // A status as an icon (a drawn one from the mod's folder), tinted with its colour.
  icon(t, name, color, size) {
    return svg(`mods/bhote/st-${name}.svg`).size(size ?? 15).flex_none().text_color(color ?? t.muted);
  }

  // The icon of the state of a CI run or a check: the running one turns.
  stateIcon(t, state, color, size) {
    if (state === "running") return div().w(size ?? 15).flex_none().text_color(color ?? t.working).frames(SPIN, 90);
    const name = { ok: "ok", failed: "failed", approval: "approval", canceled: "canceled", skipped: "canceled", queued: "waiting" }[state] ?? "waiting";
    return this.icon(t, name, color, size);
  }

  // A click that closes the ... popup (a click beside it) must not also be taken by the card under it: the popup instance notes when it
  // ends, and for a moment after that the cards ignore clicks.
  menuRecent() {
    try { return Date.now() - (shared("bhote.menuClosed") ?? 0) < 400; } catch (_e) { return false; }
  }

  guard(fn) {
    return (e, cx) => { if (!this.menuRecent()) return fn(e, cx); };
  }

  async close() {
    if (this.menu) { setShared("bhote.menuClosed", Date.now()); setShared("bhote.menuOpenAt", 0); }
  }

  // The place of the button that was pressed (window coordinates): the menu opens right under it.
  // Where a trigger is, in window pixels, as a box { left, top, right, bottom }. The mouse event says where the press was in the window (position)
  // and in the element (local_position), so the element's own corner follows from the two, whatever pane it is in (the left pane lies under the
  // brand row, the right one beside the middle); its size comes in design pixels, and the zoom lies between design and window pixels.
  anchorOf(e) {
    const b = e?.bounds, p = e?.position, l = e?.local_position;
    if (!b || !p || !l) return null;
    // (the press position is in window pixels already: only the size of the element comes in design pixels)
    const z = Number(settings()?.zoom) || 1;
    const left = p.x - l.x, top = p.y - l.y;
    return { left, top, right: left + b.width / z, bottom: top + b.height / z };
  }

  // THE dropdown: every menu opens through here, under (or over) the trigger it belongs to, never by a position worked out on its own.
  //   anchor    the box of the trigger (anchorOf of its mouse press); none: where the pointer is
  //   placement "below-right" (the default: right edges together), "below-left", "above-right", "above-left"
  // The height follows the items (a head is lower than an item); a long menu scrolls.
  openMenu(items, anchor, o = {}) {
    if (!items.length) return;
    const heads = items.filter((it) => it.type === "head").length;
    const width = o.width ?? 224;
    const height = Math.min(380, Math.round(heads * 28 + (items.length - heads) * 31 + 14));
    let at = "pointer";
    if (anchor) {
      const place = o.placement ?? "below-right";
      const z = Number(settings()?.zoom) || 1;   // (the popup's size is in design pixels too: it is drawn zoom times that on the screen)
      const x = place.endsWith("left") ? anchor.left : anchor.right - width * z;
      const y = place.startsWith("above") ? anchor.top - height * z - 4 : anchor.bottom + 4;
      at = { x: Math.max(8, Math.round(x)), y: Math.max(8, Math.round(y)) };
    }
    try { setShared("bhote.menuOpenAt", Date.now()); } catch (_e) { /* the panels keep their hover */ }
    openOverlay({ mod: "bhote", props: { menu: items }, kind: "popup", width, height, at });
  }

  menuView() {
    const t = theme();
    return v_flex().id("menu-list").size_full().p(4).bg(t.popover).text_size(12.5).overflow_y_scrollbar().children(this.menu.map((it, i) => it.type === "head"
      ? div().px(10).pt(6).pb(2).text_size(11).text_color(t.dim).child(it.label.toUpperCase())
      : h_flex().id(`mi-${i}`).gap(9).items_center().px(10).py(6).rounded(5).text_color(it.dim ? t.dim : it.danger ? t.blocked : t.foreground)
        .when(!it.dim, (el) => el.cursor_pointer().hover((e) => e.bg(t.hover)).on_click((_e, cx) => this.runItem(it, cx)))
        .when(it.check !== undefined, (el) => el.child(checkbox(t, !!it.check)))
        .child(div().flex_1().min_w(0).truncate().child(it.label))));
  }

  // What an item does; the popup closes after it (a started exec keeps running).
  runItem(it, cx) {
    // a multi-choice item: it ticks or unticks and the menu stays open
    if (it.type === "prtoggle") {
      const cur = this.prFilter();
      if (!it.value) cur.clear(); else if (cur.has(it.value)) cur.delete(it.value); else cur.add(it.value);
      setShared("bhote.prFilter", [...cur]);
      this.menu = this.menu.map((m) => (m.type === "prtoggle" ? { ...m, check: m.value ? cur.has(m.value) : cur.size === 0 } : m));
      if (cx) cx.notify();
      return;
    }
    if (it.type === "url") exec("open", [it.url]).catch(() => {});
    else if (it.type === "status") { this.markPending([it.id], { status: it.status }); exec("bhote", [it.status, it.id], { timeout: 30000 }).catch(() => {}); }
    else if (it.type === "create") exec("bhote", ["add", it.title, "-d", it.desc, "-s", "next"], { timeout: 30000 }).catch(() => {});
    else if (it.type === "focus") run("agent.focus", { terminal: it.terminal });
    else if (it.type === "stash") {
      const list = this.stashList(), x = list.find((q) => q.id === it.id);
      if (x) {
        this.setStash(list.filter((q) => q.id !== it.id));
        if (it.op === "send") exec("hc", ["send", it.terminal, x.text], { timeout: 20000 }).catch(() => {});
        else if (it.op === "topic") {
          const cut = x.text.length > 80 ? x.text.lastIndexOf(" ", 80) : -1;
          const title = (cut > 20 ? x.text.slice(0, cut) : x.text.slice(0, 80)).trim(), rest = x.text.slice(title.length).trim().slice(0, 140);
          exec("bhote", ["add", title, ...(rest ? ["-d", rest] : []), "-s", it.status], { timeout: 30000 }).catch(() => {});
        }
      }
    }
    else if (it.type === "newspace") run("space.new", { machine: it.machine, ...(it.name ? { name: it.name } : {}), ...(it.folder ? { folder: it.folder } : {}) });
    else if (it.type === "bstatus") { this.markPending(it.ids, { status: it.status }); for (const id of it.ids) exec("bhote", [it.status, id], { timeout: 90000 }).catch(() => {}); this.setSel([], this.sel().tests); }
    else if (it.type === "bdelete") { this.markPending(it.ids, { deleted: true }); for (const id of it.ids) exec("bhote", ["rm", id], { timeout: 90000 }).catch(() => {}); this.setSel([], this.sel().tests); }
    else if (it.type === "bhide") {
      const h = shared("bhote.hidden") ?? { archived: [], discarded: [] };
      const a = new Set(h.archived ?? []), d = new Set(h.discarded ?? []);
      for (const k of it.keys) {
        if (it.mode === "archive") a.add(k);
        else if (it.mode === "discard") { d.add(k); a.delete(k); }
        else if (it.mode === "restore") a.delete(k);
      }
      setShared("bhote.hidden", { archived: [...a].slice(-500), discarded: [...d].slice(-1000) });
      this.setSel(this.sel().topics, []);
    }
    else if (it.type === "bsel") {
      const cur = this.sel();
      const next = it.op === "all" ? new Set(it.keys) : new Set();
      if (it.kind === "topics") this.setSel(next, cur.tests); else this.setSel(cur.topics, next);
    }
    else if (it.type === "bask") {   // a question first: the menu is replaced by its own items
      const o = props()?.overlay; if (o) closeOverlay(o);
      this.openMenu(it.items, it.anchor ?? null);
      return;
    }
    else if (it.type === "pickstart") this.startTopic(it.id, it.machine, it.pane, it.name);
    else if (it.type === "queue") setShared("bhote.queue", [...this.queueNow().filter((e) => e.id !== it.id), { id: it.id, machineName: it.machineName, machine: it.machine, pane: it.pane, name: it.name, at: Date.now() }]);
    else if (it.type === "unqueue") setShared("bhote.queue", this.queueNow().filter((e) => e.id !== it.id));
    else if (it.type === "hide") {
      const h = shared("bhote.hidden") ?? { archived: [], discarded: [] };
      const a = new Set(h.archived ?? []), d = new Set(h.discarded ?? []);
      if (it.mode === "archive") a.add(it.key);
      else if (it.mode === "discard") { d.add(it.key); a.delete(it.key); }
      else if (it.mode === "restore") a.delete(it.key);
      setShared("bhote.hidden", { archived: [...a].slice(-500), discarded: [...d].slice(-1000) });
    }
    else if (it.type === "show") {
      setShared("bhote.show", it.id);
      run("layout.show", { id: "panel" });
      run("layout.select", { stack: "panel-body", child: "bhote-right" });
    }
    const o = props()?.overlay;
    if (o) closeOverlay(o);
  }

  head(t, text, right, keepCase) {
    return h_flex().px(10).pt(10).pb(4).items_center().text_size(11).font_semibold().text_color(t.dim)
      .child(div().flex_1().child(keepCase ? text : text.toUpperCase()))
      .when(!!right, (el) => el.child(right));
  }

  // The live agent of a topic: its name is the space's name, its machine the space's machine.
  agentOf(topic, list, machineOf) {
    if (!topic.agent) return null;
    return list.find((a) => {
      if (norm(a.space_name) !== norm(topic.agent)) return false;
      const m = machineOf(a);
      return m ? !isLocalName(topic.machine) && m.toLowerCase().startsWith(topic.machine.toLowerCase()) : isLocalName(topic.machine);
    }) ?? null;
  }

  // The state word and colour of a card, from its status and its agent.
  cardState(t, topic, agent) {
    const c = { orange: t.working, blue: t.ansi4, cyan: t.ansi6, green: t.done, yellow: t.ansi3, red: t.blocked, dim: t.dim };
    const g = topic.group;
    if (g === "review") return { word: "review", color: c.blue, bar: c.blue };
    if (g === "testing") return { word: "tests running", color: c.cyan, bar: c.cyan };
    if (g === "deploying") return { word: "waiting for deployment", color: c.yellow, bar: c.yellow };
    if (g === "finished") return { word: "finished", color: c.green, bar: c.green };
    if (g === "next") return { word: "next", color: c.yellow, bar: c.yellow };
    if (g === "waiting") return { word: topic.waiting_for ? `waiting for ${topic.waiting_for}` : "waiting", color: c.yellow, bar: c.yellow };
    if (g === "later") return { word: "parked", color: c.dim, bar: c.dim };
    if (g === "done") return { word: "done", color: c.green, bar: c.green };
    if (!agent) return { word: topic.agent ? "agent not here" : "at work", color: c.dim, bar: c.orange };
    if (agent.status === "working") return { word: "working", color: c.orange, bar: c.orange };
    if (agent.status === "blocked") return { word: "needs you", color: c.red, bar: c.red };
    if (agent.status === "done") return { word: "finished", color: c.green, bar: c.green };
    return { word: "idle", color: c.dim, bar: c.orange };
  }

  // The icon of an agent on a card: the spinner of a working one, else a mark.
  agentIcon(t, agent, active, color, word) {
    if (!agent) return div().w(14).flex_none().text_color(t.dim).child("◌");
    const state = word ?? (agent.status === "working" ? "working" : agent.status === "blocked" ? "needs you" : agent.status === "done" ? "finished" : "idle");
    if (state === "working") return div().w(14).flex_none().text_color("#d97757").frames(SPIN, 90);   // (the card is blue, the spinner Claude orange)
    if (state === "needs you") return div().w(14).flex_none().text_color(color ?? t.blocked).child("◉");
    if (state === "finished") return div().w(14).flex_none().text_color(color ?? t.done).child("✓");
    return div().w(14).flex_none().text_color(active ? t.muted : t.dim).child("○");
  }

  // The card of an agent (or of a space without one) in the left panel. The machines are headings, so a card does not repeat its machine.
  // Three rows at most, always in this order, the text of the last two under the name:
  //   status mark · name ······ badges (monitor) · what it says (working, needs you, finished; an idle agent says nothing)
  //   what it works on (its task)
  //   ⎇ branch
  // The active card has the selection ground and an accent border, every other agent a quiet surface; a space without an agent has no ground.
  card(t, id, o) {
    const { agent } = o;
    // a card whose branch is being deployed is yellow (the colour of a run under way), whatever else it does, unless the agent needs you
    const deploying = !!agent && !!o.deploy && agent.status !== "blocked";
    const st = deploying ? { word: "deploying", color: t.working, bar: t.working } : o.st;
    const active = !!o.selected;
    const dim = !!o.dim && !active;   // (an idle agent is dimmed, the active card never: its ground is light)
    const task = agent?.title ?? "";
    const word = agent ? st.word : "";   // (the state is always a badge, an idle agent too)
    const dep = deploying ? o.deploy : null;
    const ident = agent ? (o.code ?? "") : "";   // the pane code: w2:a1, an agent of another machine with its code in front (OMR/w3:a1), as bhote has always had it
    // the card takes the colour of its state (blue, red, green; an idle agent stays grey); the active one a stronger ground and a solid border
    const tone = agent && st.word !== "idle" ? st.color : null;
    // a card that has just finished lights up (the first 1.5 s), stays green, and fades to idle over its last 4 s
    const badgeColor = tone ?? (active ? t.muted : t.dim);
    const fa = o.finAge;
    const hx = (n) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, "0");
    const ease = (x) => x * x * (3 - 2 * x);   // (smoothstep: slow at both ends)
    const fade = fa == null ? 1 : fa < FLASH_MS ? 1 + 1.4 * ease(1 - fa / FLASH_MS) : fa > FINISH_MS - FADE_MS ? ease(Math.max(0, (FINISH_MS - fa) / FADE_MS)) : 1;
    const ground = o.flat ? t.chrome : tone ? tint(tone, hx((active ? 0x3a : 0x1c) * fade), t.surface) : active ? tint(t.accent, "40", t.selected) : t.surface;
    // the active card has a real blue border, whatever the agent does (the colour of its state stays on the ground, the badge and the mark)
    const BLUE = "#3b82ff";
    const edge = o.flat ? (active ? BLUE : t.chrome) : active ? BLUE : tone ? tint(tone, hx(0x70 * fade * (fade > 1 ? 1.7 : 1)), t.border) : t.border;
    const indent = 22;   // the mark (14) and its gap (8): the rows under the name start where the name starts
    return h_flex().id(id).bg(ground).border(active ? 2 : 1).border_color(edge)
      .hover((x) => x.bg(o.flat ? t.surface : tone ? tint(tone, active ? "4a" : "2a", t.surface) : active ? tint(t.accent, "52", t.selected) : t.raised)).mx(8).mb(6).rounded(8)
      .on_click(this.guard(() => { if (o.onClick) o.onClick(); else if (agent) run("agent.focus", { terminal: agent.address }); }))
      .child(v_flex().flex_1().min_w(0).gap(2).px(active ? 10 : 11).py(active ? 7 : 8)
        .child(h_flex().gap(8).items_center()
          .child(this.agentIcon(t, agent, active, tone, st.word))
          .child(div().flex_1().min_w(0).truncate().font_semibold().text_color(dim ? t.dim : t.foreground).child(o.name))
          .when(!!agent && !!o.listen?.key, (el) => el.child(this.ibtn(t, `ear-${id}`, o.listen.on ? "ear" : "ear-off", (cx) => o.onListen(cx),
            { w: 22, size: 14, color: o.listen.on ? (active ? t.muted : t.dim) : t.ansi3, hover: o.listen.on ? t.foreground : t.ansi3 })))
          .when(!!agent && (o.listen?.waiting ?? 0) > 0, (el) => el.child(div().flex_none().px(5).h(15).flex().items_center().rounded(8).text_size(10).font_semibold().text_color(t.ansi3)
            .bg(tint(t.ansi3, "26", t.raised)).border(1).border_color(tint(t.ansi3, "66", t.border)).child(`${o.listen.waiting}`)))   // (messages that wait: ear off, or the agent at work)
          .when(!!o.monitor, (el) => el.child(div().flex_none().px(6).rounded(8).text_size(10).text_color(t.accent).bg(tint(t.accent, "26", t.raised)).border(1).border_color(tint(t.accent, "55", t.border))
            .child(o.monitor.length > 1 ? `monitor ${o.monitor.length}` : `monitor · ${(o.monitor[0].every ?? "").replace(/^Every /i, "").replace(" minutes", "m").replace(" minute", "m").replace(" hours", "h").replace(" hour", "h")}`)))
          .when(!!word, (el) => el.child(div().flex_none().px(6).py(1).rounded(5).text_size(9.5).font_semibold().text_color(badgeColor)
            .bg(tint(badgeColor, "22", t.surface)).border(1).border_color(tint(badgeColor, "55", t.border)).child(word.toUpperCase()))))
        .when(!!task || !!ident, (el) => el.child(h_flex().gap(8).items_center().pl(indent)
          .child(div().flex_1().min_w(0).truncate().text_size(12).text_color(active ? t.foreground : dim ? t.dim : t.muted).child(task || " "))
          .when(!!ident, (e) => e.child(div().flex_none().text_size(10.5).text_color(active ? t.muted : t.dim).child(ident)))))
        .when(!!o.branch, (el) => el.child(div().truncate().pl(indent).text_size(11).text_color(active ? t.muted : t.dim).child(`⎇ ${o.branch}`)))
        // the deployment under way: a bar across the card, the percentage and the time left at its end
        .when(!!dep && dep.frac != null, (el) => el.child(h_flex().gap(8).items_center().pl(indent).pt(3)
          .child(div().flex_1().h(4).rounded(2).bg(tint(t.working, "30", t.border)).overflow_hidden().child(div().h(4).w(`${Math.round(Math.max(0, Math.min(1, dep.frac)) * 100)}%`).rounded(2).bg(t.working)))
          .child(div().flex_none().text_size(10.5).text_color(t.working).child(`${Math.round(Math.max(0, Math.min(1, dep.frac)) * 100)}%${dep.remain != null ? ` · ${left(dep.remain).replace(" left", "")}` : ""}`)))));
  }

  // A new space is a new shell on a machine (its default shell, in its home). The bar at the bottom asks which machine first (with
  // one machine there is nothing to ask); the plus next to a machine's name makes one there at once.
  chooseMachine() {
    const ms = machines().filter((m) => m.layout !== "unavailable");
    if (ms.length <= 1) { run("space.new", { machine: ms[0]?.name ?? "" }); return; }
    this.openMenu([{ type: "head", label: "New space on" }, ...ms.map((m) => ({ type: "newspace", machine: m.name ?? "", label: m.name ?? "This computer" }))],
      this.anchor, { placement: "above-left" });
  }

  // The bar at the bottom of the agent pane: it stays there while the list scrolls.
  // --- scheduled agents: systemd user timers whose service runs a headless Claude (slack monitors, depbot) ------------------------------
  // One line per job at the bottom of the left panel: its state, when it runs next, run now, pause / resume. Read and driven with
  // systemctl --user on the machine it lives on (this computer: none unless systemctl exists).
  jobScript() {
    return [
      // runs started here by hand (one line each: the unit name, adhoc, running, the description): a Linux unit, or on a Mac a process group in a pid file
      'hd="$HOME/.cache/bhote/headless"',
      'for f in "$hd"/*.pid; do [ -f "$f" ] || continue; id=$(basename "$f" .pid); p=$(cat "$f"); ps -p "$p" >/dev/null 2>&1 || continue',
      '  printf \'%s|%s|adhoc|||active|success|%s\\n\' "bhote-headless-$id" "bhote-headless-$id" "$(cat "$hd/$id.desc" 2>/dev/null)"; done',
      'command -v systemctl >/dev/null 2>&1 || exit 0',
      'for u in $(systemctl --user list-units --all --type=service --no-legend --plain "bhote-headless-*" 2>/dev/null | awk \'{print $1}\'); do',
      '  sa=$(systemctl --user show "$u" -p ActiveState --value); r=$(systemctl --user show "$u" -p Result --value); d=$(systemctl --user show "$u" -p Description --value)',
      '  printf \'%s|%s|adhoc|||%s|%s|%s\\n\' "${u%.service}" "$u" "$sa" "$r" "$d"',
      'done',
      'for t in $(systemctl --user list-units --all --type=timer --no-legend --plain 2>/dev/null | awk \'{print $1}\'); do',
      '  s=$(systemctl --user show "$t" -p Unit --value)',
      '  d=$(systemctl --user show "$s" -p Description --value)',
      '  echo "$t $d" | grep -qiE \'slack-monitor|depbot|headless\' || continue',
      '  a=$(systemctl --user show "$t" -p ActiveState --value)',
      '  n=$(systemctl --user show "$t" -p NextElapseUSecRealtime --value)',
      '  l=$(systemctl --user show "$t" -p LastTriggerUSec --value)',
      '  sa=$(systemctl --user show "$s" -p ActiveState --value)',
      '  r=$(systemctl --user show "$s" -p Result --value)',
      '  ne=""; [ -n "$n" ] && ne=$(date -d "$n" +%s 2>/dev/null)',
      '  le=""; [ -n "$l" ] && le=$(date -d "$l" +%s 2>/dev/null)',
      '  printf \'%s|%s|%s|%s|%s|%s|%s\\n\' "$t" "$s" "$a" "$ne" "$le" "$sa" "$r"',
      'done',
    ].join('\n');
  }

  loadJobs(cx) {
    if (this.jobLoading) return;
    this.jobLoading = true;
    cx.spawn(async (cx) => {
      for (const m of this.srvMachines()) {
        const key = this.srvKey(m.name);
        try {
          const r = await this.srvExec(m.name, "sh", ["-c", this.jobScript()], 20000);
          this.jobs[key] = r.stdout.split("\n").filter(Boolean).map((line) => {
            const [timer, svc, active, next, last, svcActive, result, desc] = line.split("|");
            if (active === "adhoc") {   // a headless run started from here: its description is what it was asked
              return { timer, svc, adhoc: true, id: timer.replace(/^bhote-headless-/, ""), name: desc || timer, machine: m.name, label: m.label, on: true, next: 0, last: 0,
                running: svcActive === "active" || svcActive === "activating", failed: !!result && result !== "success" };
            }
            const base = timer.replace(/\.timer$/, "");
            const name = base.includes("@") ? base.split("@")[1] : base;
            return { timer, svc, name, machine: m.name, label: m.label, on: active === "active", next: Number(next) || 0, last: Number(last) || 0,
              running: svcActive === "active" || svcActive === "activating", failed: !!result && result !== "success" };
          });
        } catch (_e) { this.jobs[key] = this.jobs[key] ?? []; }
      }
      for (const k of Object.keys(this.jobPending)) if (this.jobPending[k].done) delete this.jobPending[k];
      this.jobLoading = false;
      cx.notify();
    });
  }

  // The spaces that were there: every space seen is remembered per machine (its name and folder, when it was last seen), so that the menu of the
  // + next to a machine can offer the last five that are gone and open one again (a new space of that name in that folder).
  trackSpaces() {
    let hist = {};
    try { hist = shared("bhote.spaceHist") ?? {}; } catch (_e) { hist = {}; }
    const now = Date.now();
    let changed = false;
    const cwdOf = (s) => s.folder || (agents().filter((a) => a.space === s.id).map((a) => this.agentCwd[a.address]).find(Boolean) ?? "");
    for (const s of spaces()) {
      const key = s.machine ?? "local";
      const folder = cwdOf(s);
      const list = hist[key] ?? (hist[key] = []);
      const e = list.find((x) => x.name === s.name && x.folder === folder);
      if (!e) { list.push({ name: s.name, folder, at: now }); changed = true; }
      else if (now - e.at > 300000) { e.at = now; changed = true; }
      if (list.length > 30) { list.sort((a, b) => b.at - a.at); list.length = 30; }
    }
    if (changed) { try { setShared("bhote.spaceHist", hist); } catch (_e) { /* not remembered */ } }
  }

  // The pane code of an agent as bhote shows it: w<space number>:a<number of the agent in it>, with the code of its machine in front when it is not
  // this one (OMR/w3:a1). The space number is its place among the spaces of its machine, the machine code the first letter and the next two consonants
  // of its name (Omarchy: OMR), a name of three letters or less as it is (Mac: MAC).
  // What the card calls the agent: its name from Middle-earth (Frodo), a second agent in the same space with its place behind it (Frodo 2); the old code
  // (w2:a1) only while its record is not there yet. The key is "label of the machine/name of the space", normalised, as the records are.
  agentIdent(sp, machine, remote, spaceNo, agentNo, index) {
    const label = remote ? (this.labelOf?.(machine) ?? machine) : this.hostLabel;
    const name = this.ringMap?.get(`${label}/${sp.name}`.toLowerCase().replace(/[^a-z0-9]/g, ""));
    return name ? (index > 0 ? `${name} ${index + 1}` : name) : this.paneCode(machine, remote, spaceNo, agentNo);
  }

  paneCode(machine, remote, spaceNo, agentNo) {
    const code = (name) => {
      const n = String(name).replace(/[^A-Za-z]/g, "").toUpperCase();
      if (n.length <= 3) return n;
      const c = n.slice(1).replace(/[AEIOU]/g, "").slice(0, 2);
      return n[0] + (c.length === 2 ? c : (c + n.slice(1)).slice(0, 2));
    };
    return `${remote ? `${code(this.labelOf?.(machine) ?? machine)}/` : ""}w${spaceNo}:a${agentNo}`;
  }

  // The menu of the + next to a machine: a new space, and the last five spaces of that machine that are gone.
  openSpaceMenu(remote, anchor) {
    let hist = {};
    try { hist = shared("bhote.spaceHist") ?? {}; } catch (_e) { hist = {}; }
    const open = new Set(spaces().filter((s) => (s.machine ?? null) === (remote ?? null)).map((s) => s.name));
    const homeName = home().split("/").pop();
    const gone = (hist[remote ?? "local"] ?? []).filter((x) => !open.has(x.name) && x.name !== "~" && x.name !== homeName).sort((a, b) => b.at - a.at).slice(0, 5);
    const items = [{ type: "newspace", label: "New", machine: remote ?? "" },
      ...(gone.length ? [{ type: "head", label: "Recent" }] : []),
      ...gone.map((x) => ({ type: "newspace", label: x.name, machine: remote ?? "", name: x.name, folder: x.folder }))];
    this.openMenu(items, anchor);
  }

  // Recently finished: an agent that stopped working (herdr-app says "done" only for a moment, then idle). The moment is kept per agent in the
  // shared state, so that it survives a reload and holds for FINISHED_MINUTES; the agent working again ends it. True when something changed.
  trackFinished() {
    let fin = {};
    try { fin = shared("bhote.finished") ?? {}; } catch (_e) { fin = {}; }
    const now = Date.now();
    let changed = false;
    const live = new Set();
    for (const a of agents()) {
      live.add(a.address);
      const prev = this.prevSt[a.address];
      if (a.status === "working" || a.status === "blocked") { if (fin[a.address]) { delete fin[a.address]; changed = true; } }
      else if ((prev === "working" || (a.status === "done" && prev !== "done")) && !fin[a.address]) { fin[a.address] = now; changed = true; }
      this.prevSt[a.address] = a.status;
    }
    for (const k of Object.keys(fin)) if (!live.has(k) || now - fin[k] > FINISH_MS + 1000) { delete fin[k]; changed = true; }
    if (changed) { try { setShared("bhote.finished", fin); } catch (_e) { /* not remembered */ } }
    // while a card lights up or fades to idle the panel is drawn at the rate of the timer (smooth), otherwise twice a second while one is finished
    this.animating = Object.values(fin).some((t0) => { const age = now - t0; return age < FLASH_MS || age > FINISH_MS - FADE_MS; });
    return changed || Object.keys(fin).length > 0;
  }

  // How long ago an agent finished (ms), or null when it did not, just now: a card says "finished" for FINISH_MS and then becomes idle.
  finAge(a) {
    if (a.status === "working" || a.status === "blocked") return null;
    let fin = {};
    try { fin = shared("bhote.finished") ?? {}; } catch (_e) { fin = {}; }
    const age = fin[a.address] ? Date.now() - fin[a.address] : null;
    return age != null && age < FINISH_MS ? age : null;
  }

  isFinished(a) { return this.finAge(a) != null; }

  // The monitors of the sessions on every machine (monitors.py, copied to the other machines once) and where each agent runs, so that an agent's
  // card can say it has a monitor.
  loadMonitors(cx) {
    if (this.monLoading) return;
    this.monLoading = true;
    cx.spawn(async (cx) => {
      try {
        const ls = await exec("hc", ["ls", "--json"], { timeout: 15000 });
        for (const x of JSON.parse(ls.stdout).terminals ?? []) if (x.address && x.cwd) this.agentCwd[x.address] = x.cwd;
      } catch (_e) { /* the folders stay as they were */ }
      const mine = `${home()}/.config/herdr-app/mods/bhote/monitors.py`;
      for (const m of this.srvMachines()) {
        const key = this.srvKey(m.name);
        try {
          if (m.name != null && !this.monReady[key]) {
            await exec("sh", ["-c", `hc exec --on '${m.name}' sh -c 'mkdir -p "$HOME/.cache/bhote"' && hc file write --on '${m.name}' "$(hc exec --on '${m.name}' sh -c 'printf %s "$HOME"')/.cache/bhote/monitors.py" < '${mine}'`], { timeout: 30000 });
            this.monReady[key] = true;
          }
          const r = m.name == null ? await exec("python3", [mine], { timeout: 20000 })
            : await this.srvExec(m.name, "sh", ["-c", 'python3 "$HOME/.cache/bhote/monitors.py"'], 20000);
          this.mons[key] = JSON.parse(r.stdout);
        } catch (_e) { /* that machine's last answer stays */ }
      }
      this.monLoading = false;
      cx.notify();
    });
  }

  // The monitor jobs of the session an agent runs in (its folder), or null.
  monitorOf(a, machine) {
    const cwd = this.agentCwd[a.address];
    if (!cwd) return null;
    const jobs = (this.mons[this.srvKey(machine)] ?? []).filter((x) => x.cwd === cwd).flatMap((x) => x.jobs);
    return jobs.length ? jobs : null;
  }

  jobDo(cx, job, what) {
    const pk = `${this.srvKey(job.machine)}|${job.timer}`;
    this.jobPending[pk] = { what, at: Date.now() };
    const cmd = what === "run" ? ["start", "--no-block", job.svc] : what === "kill" ? ["stop", job.svc] : what === "pause" ? ["stop", job.timer] : ["start", job.timer];
    cx.notify();
    cx.spawn(async (cx) => {
      try {
        if (job.adhoc && what === "kill") await this.srvExec(job.machine, "bash", ["-c", HEADLESS_STOP, "_", job.id], 30000);   // (a run of your own: its unit or process group)
        else await this.srvExec(job.machine, "systemctl", ["--user", ...cmd], 20000);
      } catch (_e) { /* the poll shows what is there */ }
      if (this.jobPending[pk]) this.jobPending[pk].done = true;
      this.jobLoading = false;
      this.loadJobs(cx);
    });
  }

  // The newest log of a job's last run, in nvim in a new window. The jobs write `<name>/<date>.log` below ~/.local/state (a slack monitor
  // in slack-monitor/<name>, the others in <timer>); one on another machine is copied here first.
  jobLog(cx, job) {
    const base = job.timer.replace(/\.timer$/, "");
    const dir = base.startsWith("slack-monitor@") ? `slack-monitor/${job.name}` : base;
    cx.spawn(async () => {
      try {
        const r = job.adhoc ? await this.srvExec(job.machine, "sh", ["-c", 'printf %s "$HOME/.cache/bhote/headless/$1.log"', "_", job.id], 15000)   // (a run of your own has its log beside its description)
          : await this.srvExec(job.machine, "sh", ["-c", `ls -t "$HOME"/.local/state/${dir}/*.log 2>/dev/null | head -1`], 15000);
        const path = r.stdout.trim();
        if (!path) return;
        let file = path;
        if (job.machine != null) {
          file = `${home()}/.cache/bhote/logs/${job.machine}-${path.split("/").pop()}`;
          await exec("sh", ["-c", `mkdir -p '${home()}/.cache/bhote/logs' && hc exec --on '${job.machine}' cat '${path}' > '${file}'`], { timeout: 30000 });
        }
        await exec(`${home()}/.local/bin/nvim-window`, [file], { timeout: 15000 });
      } catch (_e) { /* no log yet */ }
    });
  }

  jobsBar(t) {
    const list = Object.values(this.jobs).flat();
    const clock = (e) => { const d = new Date(e * 1000); const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`; return d.toDateString() === new Date().toDateString() ? hm : `${["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][d.getDay()]} ${hm}`; };
    const add = div().id("headless-add").w(22).h(18).flex_none().flex().items_center().justify_center().rounded(5).text_size(14).text_color(t.dim).cursor_pointer()
      .hover((e) => e.bg(t.hover).text_color(t.foreground)).on_click(this.guard(() => this.openHeadless())).child("＋");
    return [v_flex().flex_none().pt(4).pb(3)
      .child(this.dividerCaption(t, "Headless agents", [...(list.length ? [div().flex_none().text_size(10.5).text_color(t.dim).child(String(list.length))] : []), add]))
      .children(list.map((j) => {
        const pk = `${this.srvKey(j.machine)}|${j.timer}`;
        const pend = this.jobPending[pk];
        const color = j.failed ? t.blocked : j.running || pend?.what === "run" ? t.working : j.on ? (t.ansi2 ?? t.done) : t.ansi3;
        const note = pend ? "…" : j.running ? "running" : !j.on ? "paused" : j.next ? clock(j.next) : "";
        return h_flex().id(`job-${pk}`).gap(9).items_center().px(15).h(24)
          .child(j.running || pend ? div().ml(3).w(9).flex_none().text_color(color).text_size(10).frames(SPIN, 90) : div().ml(3).w(8).h(8).rounded(4).flex_none().bg(color))
          .child(div().flex_1().min_w(0).truncate().pl(5).text_size(12).text_color(j.on ? t.foreground : t.muted).child(j.name))
          .child(div().flex_none().px(6).h(16).flex().items_center().rounded(8).text_size(10).text_color(t.dim).border(1).border_color(t.border).child(this.machineLabel(j.machine)))   // (where it runs)
          .child(div().flex_none().text_size(11).text_color(j.failed ? t.blocked : t.dim).child(j.failed && !j.running ? "failed" : note))
          .child(this.ibtn(t, `job-log-${pk}`, "log", (cx) => this.jobLog(cx, j), { color: t.muted }))
          .child(j.running
            ? this.ibtn(t, `job-kill-${pk}`, "stop", (cx) => this.jobDo(cx, j, "kill"), { dim: !!pend, color: t.muted, hover: t.blocked })   // (end the run that is going on)
            : j.adhoc ? div().w(20).flex_none() : this.ibtn(t, `job-run-${pk}`, "zap", (cx) => this.jobDo(cx, j, "run"), { dim: !!pend, color: t.accent }))
          .child(j.adhoc ? div().w(20).flex_none() : this.ibtn(t, `job-pause-${pk}`, j.on ? "pause" : "play", (cx) => this.jobDo(cx, j, j.on ? "pause" : "resume"), { dim: !!pend, color: j.on ? t.muted : (t.ansi2 ?? t.done) }));
      }))];
  }

  // --- the settings dialog ------------------------------------------------------------------------------------------------
  // What a setting is (type, default, description) hardly ever changes, but `bhote config list --json` is bash and takes seconds: it is
  // read once in the background and kept (shared state + localStorage). The values come from the config file itself, which is instant.
  cfgMeta() {
    // (a cache from before the registry changed is not used: v is the number of registered settings)
    try { const m = shared("bhote.cfgmeta"); if (m?.list?.length && m.v === SETTINGS.length) return m; } catch (_e) { /* not there */ }
    try { const m = JSON.parse(localStorage.getItem("cfgmeta") ?? "null"); if (m?.list?.length && m.v === SETTINGS.length) return m; } catch (_e) { /* not there */ }
    return null;
  }

  fetchCfgMeta(cx, then) {
    if (this.cfgFetching) return;
    this.cfgFetching = true;
    cx.spawn(async (cx) => {
      try {
        const r = await exec("bhote", ["config", "list", "--json"], { timeout: 60000 });
        const list = JSON.parse(r.stdout).map((x) => ({ key: x.key, type: x.type, default: x.default ?? null, description: x.description ?? "" }));
        const meta = { at: Date.now(), v: SETTINGS.length, list };
        try { setShared("bhote.cfgmeta", meta); } catch (_e) { /* kept in localStorage */ }
        try { localStorage.setItem("cfgmeta", JSON.stringify(meta)); } catch (_e) { /* not kept */ }
        if (then) then(meta);
      } catch (e) { if (then) then(null, String(e?.message ?? e)); }
      this.cfgFetching = false;
    });
  }

  async cfgValues() {
    const vals = {};
    try {
      const text = await Promise.resolve(readTextFile(`${home()}/.config/bhote/config`));
      for (const l of (text ?? "").split("\n")) { const k = l.indexOf("="); if (k > 0) vals[l.slice(0, k)] = l.slice(k + 1); }
    } catch (_e) { /* no file: every default */ }
    return vals;
  }

  initSettings(cx) {
    // this view is a window of its own: what the panel keeps for the machines (their aliases, the services) is read here again
    this.labels = ""; this.hostLabel = window()?.platform === "macos" ? "Mac" : "local";
    this.svcList = []; this.svcState = {}; this.srv = {}; this.srvInfo = {}; this.srvPending = {};
    this.setItems = null;     // null: loading
    this.setCat = "All";
    this.setBusy = {};        // key: true while a `bhote` call for it runs
    this.setFlash = {};       // key: "saved" | "failed"
    this.setIns = {};         // key: the InputState of a text setting
    this.setProjects = [];
    this.setTun = null;       // the tunnel entries of IMS_TUNNELS: { main, entries } (null: not read yet)
    this.setAdd = {};         // project id: the panel to add a stage is open
    this.setPick = {};        // project id: the stage type picked in it
    this.setRename = {};      // project id: its name is being edited
    this.setAdv = {};         // project id: the job filters are shown
    this.setMeta = null;
    this.setSearch = InputState.new({ placeholder: "Search settings", value: "" });
    this.setSearch.on("change", (_e, cx) => cx.notify());
    cx.spawn(async (cx) => { await this.loadTunnel(); cx.notify(); });   // (the tunnel entries of the IMS checkout, for the Servers group)
    const meta = this.cfgMeta();
    const done = (m, err) => {
      if (!m) { this.setItems = []; this.setError = err ?? "unknown error"; cx.notify(); return; }
      this.setMeta = m;
      this.reloadSettings().then(() => cx.notify());
    };
    if (meta) { done(meta); if (Date.now() - (meta.at ?? 0) > 12 * 3600 * 1000) this.fetchCfgMeta(cx, null); }
    else this.fetchCfgMeta(cx, done);
  }

  // The project records (the runbooks live there, not in the config file): read from the topic files, which is instant.
  async projectRecords() {
    try {
      const r = await exec("sh", ["-c", 'd="${XDG_DATA_HOME:-$HOME/.local/share}/bhote/topics"; for f in "$d"/proj-*.topic; do [ -f "$f" ] && { printf "\\036\\n"; cat "$f"; echo; }; done'], { timeout: 15000 });
      return parseProjects(r.stdout).sort((a, b) => a.title.localeCompare(b.title));
    } catch (_e) { return []; }
  }

  async reloadSettings() {
    const [vals, projects] = await Promise.all([this.cfgValues(), this.projectRecords(), this.loadSvcConfig()]);
    this.setProjects = projects;
    this.applySettings(vals);
  }

  // The InputState of a text setting: made once, refreshed when what was shown is still what is in the field (typed text stays).
  ensureInput(key, kind, value, placeholder) {
    const shown = kind === "secret" ? "" : value;
    if (!this.setIns[key]) {
      const st = InputState.new({ placeholder, value: shown });
      st.on("submit", (_e, cx) => this.submitInput(cx, key, st));
      st.on("change", (_e, cx) => cx.notify());
      this.setIns[key] = st;
    } else if (this.setIns[key].value() === (this.setShown?.[key] ?? "")) this.setIns[key].set_value(shown);
    (this.setShown ??= {})[key] = shown;
  }

  applySettings(vals) {
    this.labels = vals.MACHINE_LABELS ?? ""; this.hostLabel = (vals.HOST_LABEL ?? "").trim() || this.hostLabel;   // (the aliases of the machines, as the panel has them)
    const items = this.setMeta.list.filter((m) => SETTING_INFO.has(m.key)).map((m) => {   // (only what concerns the Mac client; the terminal panel is deprecated)
      const kind = m.type.split(":")[0];
      const info = SETTING_INFO.get(m.key);
      const set = Object.prototype.hasOwnProperty.call(vals, m.key);
      const value = kind === "secret" ? (set && vals[m.key] ? "(set)" : "") : set ? vals[m.key] : (m.default ?? "");
      if (["text", "list", "color", "secret"].includes(kind)) {
        this.ensureInput(m.key, kind, value, kind === "secret" ? (value === "(set)" ? "(set) – type to replace" : "not set") : kind === "list" ? "a, b, c" : kind === "color" ? "#rrggbb" : "");
      }
      return { ...m, ...info, value, set, changed: set && m.default != null && vals[m.key] !== m.default && kind !== "secret" };
    });
    // the projects and their runbooks (project records, not config keys): which projects there are, what belongs to each, how it is watched,
    // and the stages after the merge. Nothing about a project is in the code.
    const fields = [
      ["title", "Name", "How the project is called in the panel."],
      ["repos", "Repositories", "Git repositories that belong to it, comma-separated (host/owner/name), e.g. github.com/bilendo/ims."],
      ["ci", "CI source", "Where deployments and tests come from: github:owner/repo or circleci:gh/owner/repo. Detect reads it from the repository."],
      ["deploy_branches", "Deploy branches", "Branches that deploy, with their environment: develop=Staging,main=Production."],
      ["deploy_names", "Deploy workflows or jobs", "What counts as a deployment, Name=Label (empty: found automatically)."],
      ["test_names", "Test jobs", "What counts as a test run (empty: every job with test in its name)."],
      ["runbook", "Stages after the merge", "Comma-separated, e.g. Staging, Approval, Production. none: the path ends at the merge. Empty: the default for its CI."],
    ];
    this.ensureInput("__new", "text", "", "Project name");
    items.push({ key: "__new", type: "text", default: null, group: "Runbooks", sec: "Projects", name: "New project", desc: "Add a project, then give it its repositories and its runbook.", unit: "", dep: [], needs: [], order: 999,
      value: "", set: false, changed: false, source: "project", field: "__new", addLabel: "Add" });
    this.setProjects.forEach((pr, pi) => {
      fields.forEach(([f, name, desc], fi) => {
        const key = `${pr.id}:${f}`, value = f === "title" ? pr.title : (pr.raw[f] ?? "");
        this.ensureInput(key, "text", value, f === "runbook" ? (postStages(pr).join(", ") || "none") : f === "ci" ? "not watched" : f === "repos" ? "none pinned" : "automatic");
        items.push({ key, type: "text", default: null, group: "Runbooks", sec: pr.title, name, desc, unit: "", dep: [], needs: [], order: 1000 + pi * 10 + fi,
          value, set: value !== "", changed: f !== "title" && value !== "", source: "project", projId: pr.id, projTitle: pr.title, field: f, addLabel: f === "title" ? "Rename" : "",
          offHint: ["deploy_branches", "deploy_names", "test_names"].includes(f) && !pr.raw.ci ? "Needs a CI source (set it above, or Detect)" : "" });
      });
      for (const [f, ph] of [["__detail", "detail"], ["__custom", "Name of the stage"], ["__repo", "host/owner/name"], ["__branch", "branch"], ["__env", "environment"]]) this.ensureInput(`${pr.id}:${f}`, "text", "", ph);
      items.push({ key: `${pr.id}:__delete`, type: "action", default: null, group: "Runbooks", sec: pr.title, name: "Delete project", desc: "Its topics keep existing, without a project.", unit: "", dep: [], needs: [], order: 1000 + pi * 10 + 9,
        value: "", set: false, changed: false, source: "project", projId: pr.id, projTitle: pr.title, field: "__delete" });
    });
    // the servers: the services (servers.json) and the tunnel; drawn as cards, so one item stands for the group
    for (const x of this.svcList) {
      for (const [f, v] of [["name", x.name], ["port", String(x.port)], ["url", x.url ?? ""], ["start", x.start ?? ""], ["stop", x.stop ?? ""]]) this.ensureInput(`svc:${x.id}:${f}`, "text", v, f === "stop" ? "none" : f === "url" ? `http://localhost:${x.port}` : "");
    }
    for (const g of this.serverGroups()) this.ensureInput(`svc:__new:${g.m.name ?? "local"}:name`, "text", "", "Name of the service");
    for (const [f, ph] of [["host", "user@host"], ["block", "port block, e.g. linux"]]) this.ensureInput(`tun:${f}`, "text", "", ph);
    items.push({ key: "__servers", type: "action", default: null, group: "Servers", sec: "", name: "Servers", desc: "services start stop pause tunnel ssh bilendo marketing command port", unit: "", dep: [], needs: [], order: 1500, value: "", set: false, changed: false });
    items.sort((a, b) => a.order - b.order);
    this.setItems = items;
  }

  saveSetting(cx, key, value, unset) {
    if (this.setBusy[key]) return;
    const it = (this.setItems ?? []).find((x) => x.key === key);
    this.setBusy[key] = true; cx.notify();
    cx.spawn(async (cx) => {
      let ok = true;
      const args = it?.source === "project"
        ? (it.field === "__new" ? ["project", "add", value] : it.field === "__delete" ? ["project", "rm", it.projId] : it.field === "title" ? ["project", "rename", it.projId, value]
          : ["project", "set", it.projId, it.field, unset ? "" : value])
        : unset ? ["config", "unset", key] : ["config", "set", key, value];
      if (it?.field === "__new" && !value) { this.setBusy[key] = false; cx.notify(); return; }
      try { await exec("bhote", args, { timeout: 30000 }); } catch (_e) { ok = false; }
      const st = this.setIns[key];
      if (st && unset) { const m = this.setMeta.list.find((q) => q.key === key); st.set_value(it?.source === "project" ? "" : (m?.default ?? "")); }
      if (st && it?.field === "__new" && ok) st.set_value("");
      if (it?.field === "__delete") delete this.setConfirm?.[key];
      await this.reloadSettings();
      this.setBusy[key] = false;
      this.setFlash[key] = ok ? "saved" : "failed";
      cx.notify();
      cx.timer.after(2500, (cx) => { delete this.setFlash[key]; cx.notify(); });
    });
  }

  // Detect: find the CI of a project from its repository (bhote monitor detect), then show what it wrote.
  detectCi(cx, x) {
    const key = x.key;
    if (this.setBusy[key]) return;
    this.setBusy[key] = true; cx.notify();
    cx.spawn(async (cx) => {
      let ok = true;
      try { await exec("bhote", ["monitor", "detect", x.projTitle], { timeout: 120000 }); } catch (_e) { ok = false; }
      for (const f of ["ci", "deploy_branches", "deploy_names"]) delete (this.setShown ??= {})[`${x.projId}:${f}`];   // (the fields take what was written)
      await this.reloadSettings();
      this.setBusy[key] = false;
      this.setFlash[key] = ok ? "saved" : "failed";
      cx.notify();
      cx.timer.after(2500, (cx) => { delete this.setFlash[key]; cx.notify(); });
    });
  }

  // Enter in one of the small fields of a runbook card adds what it holds; any other field saves its setting.
  submitInput(cx, key, st) {
    if (key.startsWith("svc:")) { const id = key.split(":")[1]; if (id === "__new") this.addSvc(cx, key.split(":")[2]); else this.saveSvc(cx, id); return; }
    if (key.startsWith("tun:")) { this.addTunnel(cx); return; }
    const [id, f] = key.split(":");
    const pr = this.setProjects.find((x) => x.id === id);
    if (!pr || !f?.startsWith("__")) { this.saveSetting(cx, key, st.value().trim()); return; }
    if (f === "__repo") this.addRepo(cx, pr);
    else if (f === "__branch" || f === "__env") this.addBranch(cx, pr);
    else if (f === "__detail" || f === "__custom") this.addStage(cx, pr);
  }

  listOf(raw) { return (raw ?? "").split(",").map((x) => x.trim()).filter(Boolean); }

  addRepo(cx, pr) {
    const st = this.setIns[`${pr.id}:__repo`]; const v = st.value().trim();
    if (!v) return;
    st.set_value("");
    this.saveSetting(cx, `${pr.id}:repos`, [...new Set([...this.listOf(pr.raw.repos), v])].join(","));
  }

  addBranch(cx, pr) {
    const b = this.setIns[`${pr.id}:__branch`], e = this.setIns[`${pr.id}:__env`];
    const br = b.value().trim(), env = e.value().trim();
    if (!br) return;
    b.set_value(""); e.set_value("");
    const rest = this.listOf(pr.raw.deploy_branches).filter((x) => x.split("=")[0].trim() !== br);
    this.saveSetting(cx, `${pr.id}:deploy_branches`, [...rest, env ? `${br}=${env}` : br].join(","));
  }

  // A stage after the merge: the type (Build, Deploy, Staging, Production, Approval, Merge, Release or a name of its own) and, for some, a detail
  // (Merge → the branch, Build → what is built, Release → how). The runbook is stored as the text "Type (detail), Type, …".
  addStage(cx, pr, type, detail) {
    const pick = type ?? this.setPick[pr.id];
    if (!pick) return;
    const dIn = this.setIns[`${pr.id}:__detail`], cIn = this.setIns[`${pr.id}:__custom`];
    const d = (detail ?? dIn.value()).trim(), c = cIn.value().trim();
    const stage = pick === "custom" ? c : d ? `${pick} (${d})` : pick;
    if (!stage) return;
    dIn.set_value(""); cIn.set_value("");
    this.setAdd[pr.id] = false; this.setPick[pr.id] = null;
    this.saveSetting(cx, `${pr.id}:runbook`, [...postStages(pr), stage].join(", "));   // (the default stages become the explicit ones)
  }

  runbookCard(t, pr, items) {
    const get = (f) => items.find((y) => y.key === `${pr.id}:${f}`);
    const stages = postStages(pr), explicit = pr.runbook.length > 0;
    const kindOf = (l) => l.split(" (")[0];
    const color = (l) => { const k = kindOf(l);
      return k === "Build" ? (t.ansi4 ?? t.accent) : ["Deploy", "Staging", "Production"].includes(k) ? (t.ansi2 ?? t.done) : k === "Approval" ? (t.ansi3 ?? "#e0af68")
        : k === "Merge" ? (t.ansi5 ?? "#bb9af7") : k === "Release" ? (t.ansi6 ?? t.muted) : t.accent; };
    const write = (cx, list) => this.saveSetting(cx, `${pr.id}:runbook`, list.length ? list.join(", ") : "none");
    const move = (cx, i, d) => { const l = [...stages]; const j = i + d; if (j < 0 || j >= l.length) return; [l[i], l[j]] = [l[j], l[i]]; write(cx, l); };
    const tiny = (id, label, onClick) => div().id(id).w(20).h(24).flex_none().flex().items_center().justify_center().rounded(6).text_size(12).text_color(t.dim).cursor_pointer()
      .hover((e) => e.bg(t.hover).text_color(t.foreground)).on_click(onClick).child(label);
    const btn = (id, label, onClick, o = {}) => div().id(id).px(12).h(30).flex_none().flex().items_center().rounded(8).cursor_pointer().text_size(13)
      .border(1).border_color(o.primary ? t.accent : t.border).bg(o.primary ? t.accent : t.chrome).text_color(o.primary ? (t.accent_foreground ?? "#ffffff") : t.foreground)
      .hover((e) => e.bg(o.primary ? t.accent : t.hover)).on_click(onClick).child(label);
    const field = (id, key, w) => Input.new(this.setIns[`${pr.id}:${key}`]).w(w).h(28).px(9).rounded(7).border_1().border_color(t.border).bg(t.background).text_size(12.5).text_color(t.foreground);
    const arrow = () => div().flex_none().text_size(13).text_color(t.dim).child("→");
    const row = (label, ...kids) => h_flex().gap(14).items_start().child(div().flex_none().w(140).pt(5).text_size(13).text_color(t.muted).child(label))
      .child(v_flex().flex_1().min_w(0).gap(8).children(kids));
    const chip = (id, text, onX) => h_flex().id(id).gap(4).items_center().pl(10).pr(3).h(26).rounded(13).border(1).border_color(t.border).bg(t.chrome).text_size(12.5).text_color(t.foreground)
      .child(div().child(text)).child(tiny(`${id}-x`, "✕", onX));

    // header: name (or its field), CI, delete
    const renaming = !!this.setRename[pr.id];
    const ci = pr.raw.ci.startsWith("github:") ? "GitHub Actions" : pr.raw.ci.startsWith("circleci:") ? "CircleCI" : "no CI";
    const delKey = `${pr.id}:__delete`, ask = !!this.setConfirm?.[delKey];
    const header = h_flex().gap(10).items_center()
      .child(renaming
        ? h_flex().gap(8).items_center().flex_1().child(Input.new(this.setIns[`${pr.id}:title`]).w(240).h(30).px(10).rounded(8).border_1().border_color(t.accent).bg(t.background).text_size(15).text_color(t.foreground))
            .child(btn(`rn-ok-${pr.id}`, "Rename", (_e, cx) => { this.setRename[pr.id] = false; this.saveSetting(cx, `${pr.id}:title`, this.setIns[`${pr.id}:title`].value().trim()); }, { primary: true }))
            .child(btn(`rn-no-${pr.id}`, "Cancel", (_e, cx) => { this.setRename[pr.id] = false; this.setIns[`${pr.id}:title`].set_value(pr.title); cx.notify(); }))
        : h_flex().gap(10).items_center().flex_1().child(div().text_size(17).font_semibold().text_color(t.foreground).child(pr.title))
            .child(div().px(9).h(22).flex().items_center().rounded(11).text_size(12).border(1).border_color(t.border).text_color(pr.raw.ci ? t.muted : t.dim).child(ci))
            .child(tiny(`rn-${pr.id}`, "✎", (_e, cx) => { this.setRename[pr.id] = true; cx.notify(); })))
      .child(div().id(`dl-${pr.id}`).px(10).h(26).flex().items_center().rounded(7).cursor_pointer().text_size(12).border(1).border_color(ask ? t.blocked : "#00000000")
        .bg(ask ? t.blocked : t.popover).text_color(ask ? "#ffffff" : t.dim).hover((e) => e.text_color(ask ? "#ffffff" : t.blocked))
        .on_click((_e, cx) => { if (ask) this.saveSetting(cx, delKey, ""); else { (this.setConfirm ??= {})[delKey] = true; cx.notify(); cx.timer.after(4000, (cx) => { delete this.setConfirm[delKey]; cx.notify(); }); } })
        .child(ask ? "Really delete?" : "Delete"));

    // the path of a change: the stages every project has, then this project's own
    const fixed = ["Commit", "PR", "Tests", "Merge"].map((n) => h_flex().gap(6).items_center().px(10).h(30).rounded(15).border(1).border_color(t.border).bg(t.popover)
      .child(div().w(8).h(8).rounded(4).flex_none().bg(t.dim)).child(div().text_size(13).text_color(t.muted).child(n)));
    const path = [];
    fixed.forEach((f) => { path.push(f); path.push(arrow()); });
    stages.forEach((l, i) => {
      const c = color(l);
      path.push(h_flex().id(`st-${pr.id}-${i}`).gap(6).items_center().pl(11).pr(3).h(30).rounded(15).border(1).border_color(c).bg(tint(c, "22", t.surface))
        .child(div().w(8).h(8).rounded(4).flex_none().bg(c)).child(div().text_size(13).text_color(t.foreground).child(l))
        .child(tiny(`st-${pr.id}-${i}-l`, "‹", (_e, cx) => move(cx, i, -1))).child(tiny(`st-${pr.id}-${i}-r`, "›", (_e, cx) => move(cx, i, 1)))
        .child(tiny(`st-${pr.id}-${i}-x`, "✕", (_e, cx) => write(cx, stages.filter((_x, k) => k !== i)))));
      if (i < stages.length - 1) path.push(arrow());
    });
    if (!stages.length) path.push(div().text_size(13).text_color(t.dim).child("ends at the merge"));
    path.push(btn(`st-add-${pr.id}`, this.setAdd[pr.id] ? "Cancel" : "＋ Stage", (_e, cx) => { this.setAdd[pr.id] = !this.setAdd[pr.id]; this.setPick[pr.id] = null; cx.notify(); }));

    // adding a stage: pick a type; the ones with a detail ask for it
    const types = [["Build", "image or artifact, e.g. Docker"], ["Deploy", ""], ["Staging", ""], ["Production", ""], ["Approval", ""], ["Merge", "target branch, e.g. main"], ["Release", "how, e.g. Capistrano"], ["custom", ""]];
    const pick = this.setPick[pr.id];
    const adder = !this.setAdd[pr.id] ? [] : [v_flex().gap(10).p(12).rounded(10).bg(t.chrome).border(1).border_color(t.border)
      .child(h_flex().gap(6).flex_wrap().children(types.map(([ty, hint]) => { const c = color(ty === "custom" ? "x" : ty), on = pick === ty;
        return div().id(`ty-${pr.id}-${ty}`).px(12).h(28).flex().items_center().gap(6).rounded(14).cursor_pointer().text_size(13).border(1).border_color(on ? c : t.border)
          .bg(on ? tint(c, "33", t.selected) : t.surface).text_color(on ? t.foreground : t.muted).hover((e) => e.bg(on ? tint(c, "40", t.selected) : t.raised))
          .on_click((_e, cx) => { if (ty === "custom" || hint) { this.setPick[pr.id] = ty; cx.notify(); } else this.addStage(cx, pr, ty, ""); })
          .child(div().w(7).h(7).rounded(4).bg(c)).child(ty === "custom" ? "Custom…" : ty); })))
      .when(!!pick, (e) => e.child(h_flex().gap(8).items_center()
        .child(field(`ad-${pr.id}`, pick === "custom" ? "__custom" : "__detail", 280))
        .child(div().text_size(12).text_color(t.dim).child(pick === "custom" ? "name of the stage" : (types.find((x) => x[0] === pick)?.[1] ?? "")))
        .child(btn(`ad-ok-${pr.id}`, "Add", (_e, cx) => this.addStage(cx, pr), { primary: true }))))];

    const repos = this.listOf(pr.raw.repos);
    const branches = this.listOf(pr.raw.deploy_branches);
    const adv = !!this.setAdv[pr.id];
    const plain = (f) => { const it = get(f); return it ? h_flex().gap(14).items_center().child(div().flex_none().w(140).text_size(13).text_color(t.muted).child(it.name)).child(this.settingControl(t, it)) : div(); };
    return v_flex().id(`rb-${pr.id}`).gap(16).mx(24).my(10).p(18).rounded(14).border(1).border_color(t.border).bg(t.surface)
      .child(header)
      .child(v_flex().gap(10)
        .child(div().text_size(12).text_color(t.dim).child(`PATH OF A CHANGE${explicit ? "" : " · default for its CI"}`))
        .child(h_flex().gap(8).flex_wrap().items_center().children(path))
        .children(adder)
        .when(explicit, (e) => e.child(div().id(`rbdef-${pr.id}`).text_size(12).text_color(t.dim).cursor_pointer().hover((h) => h.text_color(t.foreground))
          .on_click((_e, cx) => this.saveSetting(cx, `${pr.id}:runbook`, "", true)).child("Use the default again"))))
      .child(div().h(1).bg(t.border))
      .child(plain("ci"))
      .child(row("Repositories",
        h_flex().gap(6).flex_wrap().children(repos.length ? repos.map((r, i) => chip(`rp-${pr.id}-${i}`, r, (_e, cx) => this.saveSetting(cx, `${pr.id}:repos`, repos.filter((_x, k) => k !== i).join(",")))) : [div().text_size(13).text_color(t.dim).child("none pinned")]),
        h_flex().gap(8).items_center().child(field(`rp-in-${pr.id}`, "__repo", 260)).child(btn(`rp-add-${pr.id}`, "Add", (_e, cx) => this.addRepo(cx, pr)))))
      .child(row("Deploy branches",
        h_flex().gap(6).flex_wrap().children(branches.length ? branches.map((b, i) => { const [br, env] = b.split("=");
          return chip(`br-${pr.id}-${i}`, env && env.trim() !== br.trim() ? `${br.trim()} → ${env.trim()}` : br.trim(), (_e, cx) => this.saveSetting(cx, `${pr.id}:deploy_branches`, branches.filter((_x, k) => k !== i).join(","))); })
          : [div().text_size(13).text_color(t.dim).child(pr.raw.ci ? "none" : "needs a CI source")]),
        h_flex().gap(8).items_center().child(field(`br-b-${pr.id}`, "__branch", 130)).child(div().text_color(t.dim).child("→")).child(field(`br-e-${pr.id}`, "__env", 150))
          .child(btn(`br-add-${pr.id}`, "Add", (_e, cx) => this.addBranch(cx, pr)))))
      .child(div().id(`adv-${pr.id}`).text_size(12).text_color(t.dim).cursor_pointer().hover((h) => h.text_color(t.foreground))
        .on_click((_e, cx) => { this.setAdv[pr.id] = !adv; cx.notify(); }).child(`${adv ? "▾" : "▸"} Job filters`))
      .children(adv ? [plain("deploy_names"), plain("test_names")] : []);
  }

  // --- the servers in the settings: the services and the tunnel ------------------------------------------------------------------
  async loadTunnel() {
    try {
      let main = ""; try { main = shared("bhote.imsMain") ?? ""; } catch (_e) { main = ""; }
      if (!main) {
        const r = await exec("zsh", ["-ic", 'print -r -- "main=$_ims_main"'], { timeout: 30000 });
        main = (/^main=(.*)$/m.exec(r.stdout) ?? [])[1]?.trim() ?? "";
      }
      if (!main) { this.setTun = { main: "", entries: [] }; return; }
      const r = await exec("sh", ["-c", 'grep -m1 "^IMS_TUNNELS=" "$1/server/.env" | cut -d= -f2-', "_", main], { timeout: 15000 }).catch(() => ({ stdout: "" }));
      this.setTun = { main, entries: r.stdout.trim().split(",").map((x) => x.trim()).filter(Boolean) };
    } catch (_e) { this.setTun = { main: "", entries: [] }; }
  }

  saveTunnel(cx, entries) {
    if (!this.setTun?.main) return;
    this.setTun = { ...this.setTun, entries };
    cx.notify();
    // only the IMS_TUNNELS line of the main checkout's server/.env is touched
    exec("sh", ["-c", 'f="$1/server/.env"; if grep -q "^IMS_TUNNELS=" "$f"; then V="$2" perl -i -pe \'s/^IMS_TUNNELS=.*/IMS_TUNNELS=$ENV{V}/\' "$f"; else printf "IMS_TUNNELS=%s\\n" "$2" >> "$f"; fi',
      "_", this.setTun.main, entries.join(",")], { timeout: 15000 }).then(() => this.loadTunnel()).then(() => cx.notify()).catch(() => cx.notify());
  }

  addTunnel(cx) {
    const h = this.setIns["tun:host"], b = this.setIns["tun:block"];
    const entry = `${h.value().trim()}:${b.value().trim()}`;
    if (!/^[A-Za-z0-9._-]+@[A-Za-z0-9._-]+:[A-Za-z0-9_-]+$/.test(entry)) { this.setFlash["tun"] = "failed"; cx.notify(); return; }
    h.set_value(""); b.set_value("");
    this.setFlash["tun"] = "saved";
    this.saveTunnel(cx, [...new Set([...(this.setTun?.entries ?? []), entry])]);
    cx.timer.after(2500, (cx) => { delete this.setFlash["tun"]; cx.notify(); });
  }

  svcSlug(name) {
    const base = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "service";
    let id = base, n = 2;
    while (this.svcList.some((x) => x.id === id)) id = `${base}-${n++}`;
    return id;
  }

  // A new service on one machine (mk: its name, "local" for this computer). The id carries the machine's alias, so that Bilendo here and Bilendo there are two services.
  addSvc(cx, mk) {
    const st = this.setIns[`svc:__new:${mk}:name`]; const name = st?.value().trim();
    if (!name) return;
    st.set_value("");
    const machine = mk === "local" ? null : mk;
    const id = this.svcSlug(machine ? `${name}-${this.machineLabel(machine)}` : name);
    this.writeSvcConfig([...this.svcList, { id, name, machine, port: 8080, url: "", start: "", stop: "" }]).then(() => this.reloadSettings()).then(() => cx.notify());
  }

  // The same service on another machine: the same name, port and commands, to be changed there (the paths differ, the function names usually do not).
  copySvc(cx, x, machine) {
    const id = this.svcSlug(machine ? `${x.name}-${this.machineLabel(machine)}` : x.name);
    this.writeSvcConfig([...this.svcList, { ...x, id, machine }]).then(() => this.reloadSettings()).then(() => cx.notify());
  }

  saveSvc(cx, id) {
    const x = this.svcList.find((y) => y.id === id);
    if (!x) return;
    const v = (f) => this.setIns[`svc:${id}:${f}`].value().trim();
    const port = Number(v("port"));
    if (!(port > 0 && port < 65536) || !v("name")) { this.setFlash[`svc:${id}`] = "failed"; cx.notify(); return; }
    const next = this.svcList.map((y) => (y.id === id ? { ...y, name: v("name"), port, url: v("url"), start: v("start"), stop: v("stop") } : y));
    this.setFlash[`svc:${id}`] = "saved";
    this.writeSvcConfig(next).then(() => this.reloadSettings()).then(() => cx.notify());
    cx.timer.after(2500, (cx) => { delete this.setFlash[`svc:${id}`]; cx.notify(); });
  }

  deleteSvc(cx, id) {
    this.writeSvcConfig(this.svcList.filter((y) => y.id !== id)).then(() => this.reloadSettings()).then(() => cx.notify());
  }

  serversSettings(t) {
    const field = (key, w) => Input.new(this.setIns[key]).w(w).h(30).px(10).rounded(8).border_1().border_color(t.border).bg(t.background).text_size(13).text_color(t.foreground);
    const label = (txt, w = 120) => div().flex_none().w(w).text_size(13).text_color(t.muted).child(txt);
    const btn = (id, txt, onClick, o = {}) => div().id(id).px(12).h(30).flex_none().flex().items_center().rounded(8).cursor_pointer().text_size(13)
      .border(1).border_color(o.primary ? t.accent : t.border).bg(o.primary ? t.accent : t.chrome).text_color(o.primary ? (t.accent_foreground ?? "#ffffff") : o.danger ? t.blocked : t.foreground)
      .hover((e) => e.bg(o.primary ? t.accent : t.hover)).on_click(onClick).child(txt);
    const groups = this.serverGroups();
    const cardOf = (x) => {
      const val = (f) => this.setIns[`svc:${x.id}:${f}`]?.value().trim() ?? "";
      const dirty = val("name") !== x.name || val("port") !== String(x.port) || val("url") !== (x.url ?? "") || val("start") !== (x.start ?? "") || val("stop") !== (x.stop ?? "");
      const flash = this.setFlash[`svc:${x.id}`];
      const delKey = `svc:${x.id}:__delete`, ask = !!this.setConfirm?.[delKey];
      const row = (name, ...kids) => h_flex().gap(12).items_center().child(label(name)).children(kids);
      return v_flex().id(`svcset-${x.id}`).gap(12).mx(24).my(8).p(18).rounded(14).border(1).border_color(t.border).bg(t.surface)
        .child(h_flex().gap(10).items_center()
          .child(div().flex_1().text_size(17).font_semibold().text_color(t.foreground).child(x.name))
          .when(!!flash, (e) => e.child(div().text_size(12).text_color(flash === "saved" ? (t.ansi2 ?? t.done) : t.blocked).child(flash === "saved" ? "saved" : "check the name and the port")))
          .child(div().id(`svcdel-${x.id}`).px(10).h(26).flex().items_center().rounded(7).cursor_pointer().text_size(12).border(1).border_color(ask ? t.blocked : "#00000000")
            .bg(ask ? t.blocked : t.popover).text_color(ask ? "#ffffff" : t.dim).hover((e) => e.text_color(ask ? "#ffffff" : t.blocked))
            .on_click((_e, cx) => { if (ask) this.deleteSvc(cx, x.id); else { (this.setConfirm ??= {})[delKey] = true; cx.notify(); cx.timer.after(4000, (cx) => { delete this.setConfirm[delKey]; cx.notify(); }); } })
            .child(ask ? "Really remove?" : "Remove")))
        .child(row("Name", field(`svc:${x.id}:name`, 260)))
        .child(row("Port", field(`svc:${x.id}:port`, 110), div().text_size(12).text_color(t.dim).child("it runs when this port answers")))
        .child(row("Open URL", field(`svc:${x.id}:url`, 360), div().text_size(12).text_color(t.dim).child("empty: http://localhost:<port>")))
        .child(row("Start command", field(`svc:${x.id}:start`, 360), div().text_size(12).text_color(t.dim).child("run in your dev shell (zsh -ic), detached")))
        .child(row("Stop command", field(`svc:${x.id}:stop`, 360), div().text_size(12).text_color(t.dim).child("empty: its process group and the port's listener")))
        .child(h_flex().gap(10).items_center()
          .child(btn(`svcsave-${x.id}`, "Save", (_e, cx) => this.saveSvc(cx, x.id), { primary: dirty }))
          .children(groups.filter((g) => (g.m.name ?? null) !== (x.machine ?? null) && !this.svcList.some((y) => y.name === x.name && (y.machine ?? null) === (g.m.name ?? null)))
            .map((g) => btn(`svccopy-${x.id}-${g.m.name ?? "local"}`, `Copy to ${g.m.label}`, (_e, cx) => this.copySvc(cx, x, g.m.name ?? null))))
          .child(div().text_size(12).text_color(t.dim).child("Start, stop and open it from the Servers tab. Output: ~/.cache/bhote/svc/" + x.id + ".log")));
    };
    const addOf = (g) => { const mk = g.m.name ?? "local";
      return h_flex().gap(10).items_center().mx(24).my(6)
        .child(label("New service", 120)).child(field(`svc:__new:${mk}:name`, 260)).child(btn(`svc-add-${mk}`, "Add", (_e, cx) => this.addSvc(cx, mk), { primary: true }))
        .child(div().text_size(12).text_color(t.dim).child(`on ${g.m.label}; then give it a port and its start command`)); };
    // the tunnel: ssh -L forwards to another machine's IMS systems (IMS_TUNNELS in the main checkout's server/.env: user@host:block)
    const tun = this.setTun;
    const ent = tun?.entries ?? [];
    const tflash = this.setFlash["tun"];
    const runTun = (what) => (_e, cx) => { exec("zsh", ["-ic", `_ims_node24 >/dev/null 2>&1; ims-tunnels-${what}`], { timeout: 120000 }).catch(() => {}); this.setFlash["tun"] = "saved"; cx.notify(); cx.timer.after(2500, (cx) => { delete this.setFlash["tun"]; cx.notify(); }); };
    const tunnel = v_flex().id("tunnel-settings").gap(12).mx(24).my(8).p(18).rounded(14).border(1).border_color(t.border).bg(t.surface)
      .child(h_flex().gap(10).items_center()
        .child(div().flex_1().text_size(17).font_semibold().text_color(t.foreground).child("SSH tunnel to other machines"))
        .when(!!tflash, (e) => e.child(div().text_size(12).text_color(tflash === "saved" ? (t.ansi2 ?? t.done) : t.blocked).child(tflash === "saved" ? "done" : "use user@host and a port block"))))
      .child(div().text_size(13).text_color(t.muted).child("The IMS systems of another machine appear on this one as localhost (ssh -L over its port block). Stored as IMS_TUNNELS in the main IMS checkout's server/.env."))
      .child(tun === null ? h_flex().gap(8).items_center().text_size(12).text_color(t.dim).child(div().text_color(t.working).frames(SPIN, 90)).child("reading the IMS checkout…")
        : !tun.main ? div().text_size(13).text_color(t.dim).child("No IMS checkout found in the dev shell on this computer.")
        : v_flex().gap(10)
          .child(h_flex().gap(6).flex_wrap().children(ent.length ? ent.map((e, i) => h_flex().id(`tn-${i}`).gap(4).items_center().pl(10).pr(3).h(28).rounded(14).border(1).border_color(t.border).bg(t.chrome).text_size(13).text_color(t.foreground)
              .child(div().child(e.replace(":", " · ")))
              .child(div().id(`tn-${i}-x`).w(20).h(24).flex().items_center().justify_center().rounded(6).text_size(12).text_color(t.dim).cursor_pointer()
                .hover((h) => h.bg(t.hover).text_color(t.foreground)).on_click((_e, cx) => this.saveTunnel(cx, ent.filter((_x, k) => k !== i))).child("✕")))
            : [div().text_size(13).text_color(t.dim).child("no tunnel configured")]))
          .child(h_flex().gap(8).items_center().child(label("Add", 50)).child(field("tun:host", 230)).child(field("tun:block", 200)).child(btn("tun-add", "Add", (_e, cx) => this.addTunnel(cx), { primary: true })))
          .child(h_flex().gap(8).items_center().child(btn("tun-open", "Open tunnel", runTun("start"))).child(btn("tun-close", "Close tunnel", runTun("stop")))
            .child(div().text_size(12).text_color(t.dim).child("the state shows in the Servers tab"))));
    // one block per machine (its alias as the caption), each with its services and a way to add one; the tunnel last
    return [...groups.map((g) => ({ cap: g.m.label, nodes: [...(g.svcs.length ? g.svcs.map(cardOf) : [div().mx(24).my(8).text_size(13).text_color(t.dim).child("No service on this machine yet.")]), addOf(g)] })),
      { cap: "Tunnel", nodes: [tunnel] }];
  }

  // The control of a setting: always the same slot on the right, so the rows line up.
  settingControl(t, x) {
    const [kind, ...rest] = x.type.split(":");
    const key = x.key, busy = !!this.setBusy[key];
    const save = (v) => (_e, cx) => this.saveSetting(cx, key, v);
    const box = (id) => div().id(id).h(30).flex().items_center().justify_center().rounded(7).cursor_pointer().bg(t.chrome).border(1).border_color(t.border).text_color(t.foreground).hover((e) => e.bg(t.hover));
    if (kind === "bool") {
      const on = x.value === "on";
      return div().id(`sw-${key}`).w(40).h(22).rounded(11).flex_none().cursor_pointer().flex().items_center().px(2).opacity(busy ? 0.5 : 1)
        .when(on, (e) => e.justify_end()).bg(on ? t.accent : t.raised).border(1).border_color(on ? t.accent : t.border)
        .on_click(save(on ? "off" : "on")).child(div().w(16).h(16).rounded(8).bg("#ffffff"));
    }
    if (kind === "enum") {
      return h_flex().p(2).gap(2).rounded(9).border(1).border_color(t.border).bg(t.chrome).opacity(busy ? 0.6 : 1)
        .children(rest.map((o) => { const on = x.value === o;
          return div().id(`en-${key}-${o}`).px(12).h(28).flex().items_center().rounded(7).text_size(13).cursor_pointer()
            .border(1).border_color(on ? t.border : "#00000000").bg(on ? t.raised : t.chrome).text_color(on ? t.foreground : t.muted)
            .hover((e) => e.bg(on ? t.raised : t.hover)).on_click(save(o)).child(o); }));
    }
    if (kind === "int") {
      const [lo, hi] = [Number(rest[0]), Number(rest[1])];
      const step = hi - lo > 1000 ? 60 : hi - lo > 100 ? 10 : 1;
      const cur = Number(x.value);
      const set = (v) => save(String(Math.min(hi, Math.max(lo, Number.isFinite(v) ? v : lo))));
      return h_flex().gap(8).items_center()
        .child(box(`mi-${key}`).w(30).text_size(15).on_click(set(cur - step)).child("−"))
        .child(h_flex().w(84).gap(4).items_baseline?.().justify_center().text_size(14).text_color(t.foreground)
          .child(x.value === "" || x.value == null ? "auto" : String(x.value)).when(!!x.unit && x.value !== "", (e) => e.child(div().text_size(12).text_color(t.dim).child(x.unit))))
        .child(box(`pl-${key}`).w(30).text_size(15).on_click(set(cur + step)).child("+"));
    }
    if (kind === "action") {   // a destructive button: the first click asks, the second does it
      const ask = !!this.setConfirm?.[key];
      return div().id(`ac-${key}`).px(14).h(30).flex().items_center().rounded(8).cursor_pointer().text_size(13).opacity(busy ? 0.5 : 1)
        .border(1).border_color(ask ? t.blocked : t.border).bg(ask ? t.blocked : t.chrome).text_color(ask ? "#ffffff" : t.blocked)
        .on_click((_e, cx) => { if (ask) this.saveSetting(cx, key, ""); else { (this.setConfirm ??= {})[key] = true; cx.notify(); cx.timer.after(4000, (cx) => { delete this.setConfirm[key]; cx.notify(); }); } })
        .child(busy ? "…" : ask ? "Really delete?" : "Delete");
    }
    // text, list, color, secret: a field of the same width; Enter or Save writes it
    const st = this.setIns[key];
    if (!st) return div();
    const dirty = kind === "secret" ? st.value().length > 0 : st.value().trim() !== (x.value ?? "");
    const swatch = kind === "color" && /^#[0-9a-fA-F]{6}$/.test(st.value().trim());
    return h_flex().gap(8).items_center().w(300)
      .when(swatch, (e) => e.child(div().w(22).h(22).flex_none().rounded(6).border(1).border_color(t.border).bg(st.value().trim())))
      .child(Input.new(st).flex_1().min_w(0).h(32).px(10).rounded(8).border_1().border_color(dirty ? t.accent : t.border).bg(t.background).text_size(13).text_color(t.foreground))
      .when(dirty, (e) => e.child(div().id(`sv-${key}`).px(12).h(32).flex().items_center().rounded(8).cursor_pointer().bg(t.accent).text_size(13)
        .text_color(t.accent_foreground ?? "#ffffff").on_click(save(st.value().trim())).child(busy ? "…" : "Save")))
      .when(!dirty && x.field === "ci", (e) => e.child(div().id(`dt-${key}`).px(12).h(32).flex().items_center().rounded(8).cursor_pointer().bg(t.chrome).border(1).border_color(t.border)
        .text_size(13).text_color(t.foreground).hover((h) => h.bg(t.hover)).on_click((_e, cx) => this.detectCi(cx, x)).child(busy ? "…" : "Detect")));
  }

  settingsView() {
    const t = theme();
    const close = () => { const o = props()?.overlay; if (o) closeOverlay(o); };
    const q = this.setSearch.value().trim().toLowerCase();
    const items = this.setItems ?? [];
    const loading = this.setItems === null;
    const icons = { Topics: "check", Agents: "launch", Monitors: "zap", Runbooks: "st-merged", Servers: "play", Integrations: "log", "Machines & sync": "plug" };
    const order = GROUP_ORDER;
    const cats = ["All", ...(loading ? GROUP_ORDER : order.filter((g) => items.some((x) => x.group === g)))];
    const count = (c) => (c === "Servers" ? this.svcList.length : items.filter((x) => c === "All" || x.group === c).length);
    const shown = items.filter((x) => (this.setCat === "All" || x.group === this.setCat) && (!q || `${x.key} ${x.name} ${x.desc} ${x.group} ${x.sec}`.toLowerCase().includes(q)));
    const valueOf = (k) => items.find((y) => y.key === k)?.value ?? "";
    const met = (c) => { const [k, vs] = c.split("="); const v = valueOf(k); return vs ? vs.split("|").includes(v) : v === "on" || v === "(set)"; };
    // how a requirement reads: the setting's name (an integration with its section), "name: value" for a choice
    const say = (c) => { const [k, vs] = c.split("="); const i = SETTING_INFO.get(k); const nm = i.group === "Integrations" ? `${i.sec} · ${i.name}` : i.name; return vs ? `${nm}: ${vs.split("|").join(" or ")}` : nm; };
    const joined = (l, w) => (l.length < 2 ? l.join("") : `${l.slice(0, -1).join(", ")} ${w} ${l[l.length - 1]}`);
    const rowOf = (x) => {
      const flash = this.setFlash[x.key];
      const unmet = x.dep.filter((c) => !met(c));
      const off = unmet.length > 0 || !!x.offHint;                     // (a sub-setting whose parent is off: dimmed)
      const lacking = !off && x.needs.length > 0 && !x.needs.some(met);   // (a feature without any source: a warning)
      const hint = x.offHint ? x.offHint : off ? `Needs ${joined(unmet.map(say), "and")}` : lacking ? `Needs ${joined(x.needs.map(say), "or")}` : "";
      const target = x.offHint ? null : SETTING_INFO.get((off ? unmet : x.needs)[0]?.split("=")[0] ?? "")?.group;
      return h_flex().id(`set-${x.key}`).gap(20).px(24).py(14).items_center().border_b(1).border_color(t.border).opacity(off ? 0.5 : 1)
        .child(v_flex().flex_1().min_w(0).gap(3)
          .child(h_flex().gap(8).items_center()
            .child(div().text_size(15).text_color(t.foreground).child(x.name))
            .when(x.changed, (e) => e.child(div().w(6).h(6).rounded(3).flex_none().bg(t.accent)))
            .when(!!flash, (e) => e.child(div().text_size(12).text_color(flash === "saved" ? (t.ansi2 ?? t.done) : t.blocked).child(flash === "saved" ? "saved" : "failed"))))
          .child(div().text_size(13).text_color(t.muted).child(x.desc))
          .when(!!hint, (e) => e.child(div().id(`need-${x.key}`).text_size(12).text_color(lacking ? t.ansi3 : t.dim).cursor_pointer()
            .hover((h2) => h2.text_color(t.foreground)).on_click((_e, cx) => { if (target) { this.setCat = target; this.setSearch.set_value(""); cx.notify(); } }).child(`${lacking ? "⚠ " : ""}${hint} →`))))
        .child(div().flex_none().flex().justify_end().child(this.settingControl(t, x)))
        .child(div().flex_none().w(46).flex().justify_end()
          .when(x.changed, (e) => e.child(div().id(`rs-${x.key}`).px(8).h(24).flex().items_center().rounded(6).text_size(12).text_color(t.dim).cursor_pointer()
            .hover((h) => h.bg(t.hover).text_color(t.foreground)).on_click((_e, cx) => this.saveSetting(cx, x.key, "", true)).child("reset"))));
    };
    const caption = (text) => div().px(24).pt(22).pb(6).text_size(12).text_color(t.dim).child(text.toUpperCase());
    const body = [];
    if (loading) {
      for (let i = 0; i < 6; i++) body.push(h_flex().gap(20).px(24).py(16).items_center().border_b(1).border_color(t.border)
        .child(v_flex().flex_1().gap(8).child(div().w(150 + (i % 3) * 40).h(14).rounded(4).bg(t.raised)).child(div().w(300 + (i % 2) * 120).h(11).rounded(4).bg(t.hover)))
        .child(div().w(90).h(26).rounded(8).bg(t.raised)));
    } else if (!shown.length) body.push(div().p(24).text_color(t.dim).child(this.setError ? `could not read the settings: ${this.setError}` : "no setting matches"));
    else {
      let last = null;
      const carded = new Set();
      for (const x of shown) {
        if (x.key === "__servers") {   // per machine its services (and the tunnel last): cards, not rows
          for (const g of this.serversSettings(t)) {
            const capS = this.setCat === "All" || q ? `Servers · ${g.cap}` : g.cap;
            body.push(caption(capS), ...g.nodes);
          }
          last = null;
          continue;
        }
        const proj = x.group === "Runbooks" && x.projId;
        const cap = proj ? (this.setCat === "All" || q ? "Runbooks · Projects" : "Projects") : this.setCat === "All" || q ? (x.sec ? `${x.group} · ${x.sec}` : x.group) : x.sec;
        if (cap !== last) { if (cap) body.push(caption(cap)); last = cap; }
        if (proj) {   // a project is one card with its path, not one row per field
          if (carded.has(x.projId)) continue;
          carded.add(x.projId);
          const pr = this.setProjects.find((y) => y.id === x.projId);
          if (pr) body.push(this.runbookCard(t, pr, items));
          continue;
        }
        body.push(rowOf(x));
      }
    }
    return h_flex().id("settings-root").size_full().items_start().bg(t.popover).text_size(13)
      .child(v_flex().flex_none().w(236).h_full().p(14).gap(3).border_r(1).border_color(t.border)
        .child(div().px(10).pt(8).pb(14).text_size(21).font_semibold().text_color(t.foreground).child("Settings"))
        .children(cats.map((c) => { const sel = this.setCat === c;
          return h_flex().id(`cat-${c}`).gap(12).px(12).h(38).items_center().rounded(8).cursor_pointer().text_size(14)
            .bg(sel ? t.selected : t.popover).text_color(sel ? t.foreground : t.muted).hover((e) => e.bg(sel ? t.selected : t.hover))
            .on_click((_e, cx) => { this.setCat = c; cx.notify(); })
            .child(svg(`mods/bhote/${icons[c] ?? (c === "All" ? "grid" : "dots")}.svg`).size(16).text_color(sel ? t.foreground : t.dim))
            .child(div().flex_1().child(c === "All" ? "All settings" : c))
            .when(!loading, (e) => e.child(div().text_size(12).text_color(t.dim).child(String(count(c))))); }))
        .child(div().flex_1())
        .child(h_flex().id("set-file").px(14).h(38).items_center().rounded(8).cursor_pointer().border(1).border_color(t.border).bg(t.surface).text_size(13).text_color(t.foreground)
          .hover((e) => e.bg(t.raised)).on_click(() => exec("open", [`${home()}/.config/bhote/config`]).catch(() => {})).child("Open config file")))
      .child(v_flex().flex_1().min_w(0).h_full()
        .child(h_flex().flex_none().gap(10).items_center().px(24).pt(18).pb(8)
          .child(Input.new(this.setSearch).flex_1().min_w(0).h(34).px(12).rounded(8).border_1().border_color(t.border).bg(t.background).text_size(13).text_color(t.foreground))
          .when(loading, (e) => e.child(h_flex().gap(8).items_center().text_size(12).text_color(t.dim).child(div().text_color(t.working).frames(SPIN, 90)).child("first load: reading the settings (once)…")))
          .child(div().id("set-close").w(30).h(30).flex_none().flex().items_center().justify_center().rounded(7).cursor_pointer().text_size(16).text_color(t.muted)
            .hover((e) => e.bg(t.hover).text_color(t.foreground)).on_click(close).child("✕")))
        .child(v_flex().id("set-list").flex_1().min_h(0).overflow_y_scrollbar().children(body)));
  }

  // A divider line with its caption on it (like the legend of a frame): CAPTION ───────── what belongs to it at the right.
  dividerCaption(t, text, right = []) {
    return h_flex().flex_none().gap(8).px(10).h(22).items_center()
      .child(div().text_size(10.5).font_semibold().text_color(t.dim).child(text.toUpperCase()))
      .child(div().flex_1().h(1).bg(t.border))
      .children(right);
  }

  // --- another headless agent: where, in which folder, what it is asked --------------------------------------------------------
  openHeadless() { openOverlay({ mod: "bhote", props: { headless: true }, kind: "dialog", modal: true, width: 600, height: 470 }); }

  initHeadless(cx) {
    this.labels = ""; this.hostLabel = window()?.platform === "macos" ? "Mac" : "local";
    this.hdPick = null;       // the machine (its name; null: this computer)
    this.hdBusy = false; this.hdError = "";
    this.hdDir = InputState.new({ placeholder: "folder, e.g. ~/Development/IMS", value: "~" });
    this.hdCmd = InputState.new({ placeholder: "claude -p", value: "claude -p" });
    this.hdPrompt = TextareaState.new({ placeholder: "What should it do?", value: "", rows: 6 });
    this.hdPrompt.on("change", (_e, cx) => cx.notify());
    this.cfgValues().then((v) => { this.labels = v.MACHINE_LABELS ?? ""; this.hostLabel = (v.HOST_LABEL ?? "").trim() || this.hostLabel; cx.notify(); });
  }

  startHeadless(cx) {
    const prompt = this.hdPrompt.value().trim(), dir = this.hdDir.value().trim(), cmd = this.hdCmd.value().trim();
    if (!prompt || !dir || !cmd || this.hdBusy) return;
    this.hdBusy = true; this.hdError = ""; cx.notify();
    const id = `h${Date.now().toString(36)}`;
    const desc = prompt.replace(/\s+/g, " ").slice(0, 70);
    cx.spawn(async (cx) => {
      try {
        await this.srvExec(this.hdPick, "bash", ["-c", HEADLESS_START, "_", id, dir, cmd, prompt, desc, HEADLESS_ZSH], 30000);
        try { setShared("bhote.jobsRefresh", Date.now()); } catch (_e) { /* the list looks again within 15 s */ }
        const o = props()?.overlay; if (o) closeOverlay(o);
      } catch (e) { this.hdError = /no folder/.test(String(e?.message ?? e)) ? `no folder ${dir} on that machine` : "could not start it"; this.hdBusy = false; cx.notify(); }
    });
  }

  headlessView() {
    const t = theme();
    const ms = this.srvMachines();
    const ok = !!this.hdPrompt.value().trim() && !!this.hdDir.value().trim() && !!this.hdCmd.value().trim();
    const row = (label, ...kids) => h_flex().gap(14).items_start().child(div().flex_none().w(80).pt(6).text_size(13).text_color(t.muted).child(label)).child(v_flex().flex_1().min_w(0).gap(6).children(kids));
    const field = (st) => Input.new(st).h(32).px(10).rounded(8).border_1().border_color(t.border).bg(t.background).text_size(13).text_color(t.foreground);
    return v_flex().id("headless-root").size_full().bg(t.popover).p(24).gap(16)
      .child(div().text_size(18).font_semibold().text_color(t.foreground).child("Start a headless agent"))
      .child(div().text_size(13).text_color(t.muted).child("A run in the background with no terminal: it does what the prompt says and stops. It shows in the Headless agents bar, with its log and a stop button."))
      .child(row("Runs on", h_flex().gap(6).flex_wrap().children(ms.map((m) => { const on = this.hdPick === m.name;
        return div().id(`hd-m-${m.name ?? "local"}`).px(12).h(28).flex().items_center().rounded(14).cursor_pointer().text_size(13).border(1).border_color(on ? t.accent : t.border)
          .bg(on ? tint(t.accent, "33", t.selected) : t.chrome).text_color(on ? t.foreground : t.muted).hover((e) => e.bg(on ? tint(t.accent, "40", t.selected) : t.hover))
          .on_click((_e, cx) => { this.hdPick = m.name; cx.notify(); }).child(m.label); }))))
      .child(row("Folder", field(this.hdDir)))
      .child(row("Command", field(this.hdCmd), div().text_size(12).text_color(t.dim).child("the prompt is added as its last argument")))
      .child(row("Prompt", Textarea.new(this.hdPrompt).px(10).py(8).rounded(8).border_1().border_color(t.border).bg(t.background).text_size(13).text_color(t.foreground)))
      .child(h_flex().gap(10).items_center()
        .child(div().id("hd-start").px(16).h(34).flex().items_center().rounded(8).cursor_pointer().text_size(13).bg(ok && !this.hdBusy ? t.accent : t.raised).opacity(ok ? 1 : 0.6)
          .text_color(ok && !this.hdBusy ? (t.accent_foreground ?? "#ffffff") : t.dim).on_click((_e, cx) => this.startHeadless(cx)).child(this.hdBusy ? "Starting…" : "Start"))
        .child(div().id("hd-cancel").px(14).h(34).flex().items_center().rounded(8).cursor_pointer().text_size(13).border(1).border_color(t.border).bg(t.chrome).text_color(t.foreground)
          .hover((e) => e.bg(t.hover)).on_click(() => { const o = props()?.overlay; if (o) closeOverlay(o); }).child("Cancel"))
        .child(div().text_size(12).text_color(t.blocked).child(this.hdError)));
  }

  // The ear is not only a note for the senders: with it on, the agent takes the messages that wait for it. A message is the record msg/<node>/<name>/<id> in
  // kharka (bhote send); it is handed over, as a one-line prompt that says where to read it, when the ear of that agent is on and the agent is free (idle or
  // done, not working and not asking). With the ear off, or while it works, it waits (the number next to the ear). Then the record says delivered.
  agentOfKey(node, slug) {
    const sp = spaces();
    for (const a of agents()) {
      const s = sp.find((q) => q.id === a.space);
      if (s && (s.machine ?? this.localNode) === node && (norm(s.name) || "home") === slug) return { a, sp: s };
    }
    return null;
  }

  loadMessages(cx) {
    if (this.msgLoading) return;
    this.msgLoading = true;
    cx.spawn(async (cx) => {
      let changed = false;
      try {
        if (!this.localNode) this.localNode = (await this.kharka(["whoami"])).stdout.trim();
        const keys = (await this.kharka(["ls", "msg"])).stdout.split("\n").map((x) => x.trim()).filter(Boolean);
        const waiting = {}, busy = new Set();
        for (const k of keys) {
          if (this.msgDone.has(k)) continue;
          let rec; try { rec = JSON.parse((await this.kharka(["get", k])).stdout); } catch (_e) { continue; }
          if (rec.state !== "queued") { this.msgDone.add(k); continue; }
          const [, node, slug] = k.split("/");
          const hit = this.agentOfKey(node, slug);
          if (!hit) continue;                                              // (that agent is not here now: it waits)
          const lk = `${node}/${slug}`;
          waiting[lk] = (waiting[lk] ?? 0) + 1;
          if (!this.listensTo(hit.sp).on) continue;                        // ear off: held
          if (!(hit.a.status === "idle" || hit.a.status === "done") || busy.has(lk)) continue;   // at work: when it is free, one at a time
          busy.add(lk);
          // an agent as sender is answered by its name; a message from the person (sent from bhote, no agent behind it) is answered in the session itself
          const reply = rec.fromRef ? ` To answer: bhote send ${rec.fromRef} "..."` : " It comes from the person, not from an agent: answer here in this session.";
          const line = `Message from ${rec.from}${rec.title ? ` - ${rec.title}` : ""}. Read it with: kharka get ${k}  (the field text).${reply}`;
          try {
            await exec("hc", ["send", hit.a.address, line.replace(/\s+/g, " ")], { timeout: 20000 });
            await this.kharka(["put", k, JSON.stringify({ ...rec, state: "delivered", deliveredAt: Math.floor(Date.now() / 1000) })]);
            this.msgDone.add(k); waiting[lk] -= 1; changed = true;
          } catch (_e) { /* the next round tries again */ }
        }
        if (JSON.stringify(waiting) !== JSON.stringify(this.msgWaiting)) { this.msgWaiting = waiting; changed = true; }
      } catch (_e) { /* no kharka here: nothing to hand over */ }
      this.msgLoading = false;
      if (changed) cx.notify();
    });
  }

  // --- listening: may other agents message this agent? ---------------------------------------------------------------------------
  // Agents send each other messages through kharka (the text under handoff/…, a one-line prompt through herdr.commands). Whether an agent takes them
  // is the record listen/<kharka node>/<name of its space, letters and digits in lower case> ({"listening": false}); no record = AGENT_LISTEN
  // (on unless the config says off). The switch is the ear on the agent's card; an agent that wants to message another one reads the record first.
  kharka(args) { return exec(this.kharkaBin || "kharka", args, { timeout: 15000 }); }

  listenKey(sp) {
    if (!sp?.name || (sp.machine == null && !this.localNode)) return null;
    return `${sp.machine ?? this.localNode}/${norm(sp.name) || "home"}`;   // (a space called "~" is home)
  }

  listensTo(sp) {
    const key = this.listenKey(sp);
    const rec = key ? this.listenMap[key] : null;
    return { key, on: rec ? rec.listening !== false : this.listenDefault, waiting: key ? (this.msgWaiting[key] ?? 0) : 0 };
  }

  loadListen(cx) {
    if (this.listenLoading) return;
    this.listenLoading = true;
    cx.spawn(async (cx) => {
      try {
        if (!this.localNode) this.localNode = (await this.kharka(["whoami"])).stdout.trim();
        const keys = (await this.kharka(["ls", "listen"])).stdout.split("\n").map((x) => x.trim()).filter(Boolean);
        const map = {};
        await Promise.all(keys.map(async (k) => { try { map[k.replace(/^listen\//, "")] = JSON.parse((await this.kharka(["get", k])).stdout); } catch (_e) { /* a record that cannot be read counts as none */ } }));
        if (JSON.stringify(map) !== JSON.stringify(this.listenMap)) { this.listenMap = map; cx.notify(); }
      } catch (_e) { /* no kharka here: every agent listens (the default) */ }
      this.listenLoading = false;
      cx.notify();
    });
  }

  toggleListen(cx, sp) {
    const { key, on } = this.listensTo(sp);
    if (!key) return;
    const next = !on;
    if (next === this.listenDefault) delete this.listenMap[key]; else this.listenMap[key] = { listening: next, name: sp.name };   // (at once; the next look confirms)
    cx.notify();
    const args = next === this.listenDefault ? ["del", `listen/${key}`]
      : ["put", `listen/${key}`, JSON.stringify({ listening: next, name: sp.name, at: Math.floor(Date.now() / 1000), by: "herdr-app" })];
    this.kharka(args).catch(() => { this.listenLoading = false; this.loadListen(cx); });
  }

  // --- Commit-Inspector: the git commits of the agent that has the focus ----------------------------------------------------------
  // The focused agent and where its work is. The folder `hc ls` reports for a terminal can be the one it was started in (the agent of IMS-wt2
  // reported IMS-wt1), so the folder is not taken as it is: the candidates (the live folder of the focused terminal, the folder of the space, the
  // folder of `hc ls`) only lead to the repository, and the worktree that has the branch of the space is the one that is read.
  focusAgent() {
    const a = agents().find((x) => x.focused);
    if (!a) return null;
    const sp = spaces().find((x) => x.id === a.space);
    let fo = null; try { fo = focused(); } catch (_e) { fo = null; }
    const live = fo && (fo.terminal === a.address || fo.agent === a.address) ? fo.cwd : null;
    const cands = [...new Set([live, sp?.folder, this.agentCwd[a.address]].filter(Boolean))];
    if (!cands.length) return null;
    const machine = sp?.machine ?? null, branch = sp?.branch ?? null;
    const rkey = `${this.srvKey(machine)}|${a.address}|${branch ?? ""}`;
    const dir = this.commitDir?.[rkey];
    return { a, cands, machine, branch, rkey, dir: dir?.dir ?? null, failed: !!dir && !dir.dir, key: dir?.dir ? `${this.srvKey(machine)}|${dir.dir}` : null };
  }

  // The folder the program in an agent's terminal works in (hc get: foreground.cwd); the agent of IMS-wt1 sits in a shell that started in the home folder.
  async fgCwd(a) {
    const r = await exec("hc", ["get", a.address], { timeout: 8000 });
    return JSON.parse(r.stdout).foreground?.cwd ?? null;
  }

  async resolveDir(f) {
    let first = null;
    for (const c of f.cands) {
      let out;
      try { out = (await this.srvExec(f.machine, "git", ["-C", c, "worktree", "list", "--porcelain"], 10000)).stdout; } catch (_e) { continue; }   // (not a repository)
      if (!first) first = c;
      if (!f.branch) break;
      for (const block of out.split("\n\n")) {
        const path = /^worktree (.+)$/m.exec(block)?.[1], br = /^branch refs\/heads\/(.+)$/m.exec(block)?.[1];
        if (path && br === f.branch) return path;
      }
    }
    return first;
  }

  loadCommits(cx) {
    const f = this.focusAgent();
    if (!f || this.commitLoading) return;
    const c = f.key ? this.commits[f.key] : null;
    // fresh enough: 1.5 s here, 3 s on another machine; at once when the agent has changed (it has just worked, maybe committed)
    if (c && c.seq === f.a.seq && Date.now() - c.at < (f.machine ? 3000 : 1500)) return;
    this.commitLoading = true;
    cx.spawn(async (cx) => {
      let next, key = f.key;
      try {
        let dir = f.dir;
        const rec = this.commitDir?.[f.rkey];
        if (!dir || Date.now() - (rec?.at ?? 0) > 30000) {   // (looked at again every 30 s: the agent may change into another folder)
          const fg = await this.fgCwd(f.a).catch(() => null);   // where the running program works: `hc ls` only knows the folder the shell was started in
          dir = await this.resolveDir({ ...f, cands: [...new Set([fg, ...f.cands].filter(Boolean))] });
          (this.commitDir ??= {})[f.rkey] = { dir, at: Date.now() };
          if (!dir) throw new Error("no repository");
          key = `${this.srvKey(f.machine)}|${dir}`;
        }
        const r = await this.srvExec(f.machine, "sh", ["-c", COMMITS_SH, "_", dir], 10000);
        const [log, rest = ""] = r.stdout.split("@@@base\n");
        const [baseText = "", st = ""] = rest.split("@@@status\n");
        const one = (l) => { const [hash, subject, time, author] = l.split("\x1f"); return { hash, subject, time: Number(time), author }; };
        const list = log.split("\n").filter(Boolean).map(one);
        const [bl, bref, bcount] = baseText.split("\n");
        next = { at: Date.now(), seq: f.a.seq, list, base: bl ? { ...one(bl), ref: (bref ?? "").replace(/^origin\//, ""), base: true, more: Math.max(0, (Number(bcount) || 0) - list.length) } : null, dirty: st.split("\n").filter(Boolean).length, enter: {} };
      } catch (_e) {
        next = { at: Date.now(), seq: f.a.seq, list: [], dirty: 0, error: true, enter: {} };   // not a git folder (or the machine does not answer)
      }
      const old = key ? this.commits[key] : null;
      let fresh = false;
      if (old && !old.error && !next.error) {
        next.enter = { ...old.enter };   // a commit that was not in the list before comes in with a transition (not the first load)
        const had = new Set(old.list.map((x) => x.hash));
        for (const x of next.list) if (!had.has(x.hash)) { next.enter[x.hash] = Date.now(); fresh = true; }
      }
      const same = old && JSON.stringify(old.list) === JSON.stringify(next.list) && old.base?.hash === next.base?.hash && old.base?.more === next.base?.more && old.dirty === next.dirty && old.error === next.error;
      if (key) this.commits[key] = next;
      this.commitLoading = false;
      if (fresh) this.commitAnimUntil = Date.now() + 2300;
      const minute = Math.floor(Date.now() / 60000);
      if (!same || !old || this.commitMinute !== minute) { this.commitMinute = minute; cx.notify(); }   // (a new minute: "3m ago" counts on)
    });
  }

  // A click on a commit: its full message opens in a small box above the inspector (a second click on it, or the ✕, closes it).
  openCommit(cx, f, x) {
    if (this.commitSel?.key === f.key && this.commitSel?.hash === x.hash) { this.commitSel = null; cx.notify(); return; }
    this.commitSel = { key: f.key, hash: x.hash };
    const k = `${f.key}|${x.hash}`;
    cx.notify();
    if (this.commitMsg?.[k]) return;
    (this.commitMsg ??= {})[k] = "loading";
    cx.spawn(async (cx) => {
      try {
        const r = await this.srvExec(f.machine, "sh", ["-c", 'git -C "$1" show -s --format="%an%x1f%ct%x1f%B" "$2"; echo @@@; git -C "$1" show --shortstat --format= "$2"', "_", f.dir, x.hash], 10000);
        const [head, stat = ""] = r.stdout.split("@@@\n");
        const [author, time, ...msg] = head.split("\x1f");
        this.commitMsg[k] = { author, time: Number(time), msg: msg.join("\x1f").trim(), stat: stat.trim() };
      } catch (_e) { this.commitMsg[k] = { author: "", time: 0, msg: x.subject, stat: "" }; }
      cx.notify();
    });
  }

  commitDetail(t, f) {
    const sel = this.commitSel;
    if (!sel || sel.key !== f.key) return [];
    const d = this.commitMsg?.[`${sel.key}|${sel.hash}`];
    const lines = d && d !== "loading" ? d.msg.split("\n") : [];
    return [v_flex().id("commit-detail").flex_none().gap(6).px(15).py(10).border_t(1).border_color(t.border).bg(t.surface)
      .child(h_flex().gap(8).items_center()
        .child(div().text_size(11).text_color(t.accent).child(sel.hash))
        .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.dim).child(d && d !== "loading" ? [d.author, d.time ? ago(d.time) : ""].filter(Boolean).join(" · ") : ""))
        .child(div().id("commit-detail-x").w(20).h(20).flex_none().flex().items_center().justify_center().rounded(5).text_size(11).text_color(t.dim).cursor_pointer()
          .hover((e) => e.bg(t.hover).text_color(t.foreground)).on_click((_e, cx) => { this.commitSel = null; cx.notify(); }).child("✕")))
      .child(d && d !== "loading"
        ? v_flex().id("commit-msg").gap(4).max_h(170).overflow_y_scrollbar()
            .children(lines.map((l, i) => (l.trim() === "" ? div().h(4) : div().text_size(i === 0 ? 13 : 12).when(i === 0, (e) => e.font_semibold()).text_color(i === 0 ? t.foreground : t.muted).child(l))))
        : h_flex().gap(8).items_center().text_size(12).text_color(t.dim).child(div().text_color(t.working).frames(SPIN, 90)).child("loading…"))
      .when(!!d && d !== "loading" && !!d.stat, (e) => e.child(div().text_size(11).text_color(t.dim).child(d.stat)))];
  }

  commitBar(t) {
    const f = this.focusAgent();
    if (!f || f.failed) return [];
    const c = f.key ? this.commits[f.key] : null;
    if (c?.error || (c && !c.list.length && !c.base)) return [];   // (nothing to show: no box)
    const head = this.dividerCaption(t, "Commit inspector", [
      ...(f.branch ? [div().flex_none().max_w(150).truncate().text_size(11).text_color(t.muted).child(f.branch)] : []),
      ...(c && c.dirty > 0 ? [div().flex_none().text_size(11).text_color(t.ansi3).child(`${c.dirty} uncommitted`)] : [])]);
    // the rail: a line through a dot per commit, the dots at the x of the dots of the job rows below (px 15, ml 3, 8 wide)
    const line = (on) => div().w(2).flex_1().bg(on ? t.border : "#00000000");
    const now = Date.now();
    const rows = !c
      ? [h_flex().gap(9).items_center().px(15).h(24)
          .child(div().ml(3).w(8).flex_none().text_color(t.working).text_size(10).frames(SPIN, 90))
          .child(div().pl(5).text_size(12).text_color(t.dim).child("loading commits…"))]
      : c.list.length ? [...(c.base ? [c.base] : []), ...[...c.list].reverse()].map((x, i, disp) => {   // (the base it left from on top, then the oldest, the newest at the bottom, where the work goes on)
          const isHead = i === disp.length - 1, isBase = !!x.base;
          // a new commit grows in from nothing (0.45 s, slowing down) and lights up in the accent colour, which then fades (2.2 s)
          const age = c.enter[x.hash] ? now - c.enter[x.hash] : 1e9;
          const p = Math.min(1, age / 450), ease = 1 - (1 - p) * (1 - p) * (1 - p);
          const glow = age < 2200 ? Math.round(0x38 * (1 - age / 2200)).toString(16).padStart(2, "0") : null;
          const sel = this.commitSel?.key === f.key && this.commitSel?.hash === x.hash;
          return h_flex().id(`ci-${x.hash}`).gap(9).items_center().px(15).h(Math.max(1, Math.round(26 * ease))).flex_none().overflow_hidden().opacity(age < 450 ? ease : 1)
            .cursor_pointer().hover((e) => e.bg(sel ? t.selected : t.hover)).on_click((_e, cx) => this.openCommit(cx, f, x))
            .bg(sel ? t.selected : glow ? tint(t.accent, glow, t.chrome) : "#00000000")
            .child(v_flex().ml(3).w(8).h_full().flex_none().items_center()
              .child(line(i > 0)).child(div().w(8).h(8).rounded(4).flex_none().bg(isBase ? t.chrome : isHead || sel ? t.accent : t.dim).border(sel ? 2 : isBase ? 1 : 0).border_color(isBase ? t.dim : t.foreground)).child(line(i < disp.length - 1)))
            .child(div().pl(5).w(66).flex_none().truncate().text_size(11).text_color(isBase ? t.dim : t.accent).child(x.hash))
            .child(div().flex_1().min_w(0).truncate().text_size(12).text_color(isBase ? t.dim : isHead ? t.foreground : t.muted).child(isBase ? (x.more ? `${x.more} earlier · from ${x.ref || "base"}` : `from ${x.ref || "base"} · ${x.subject}`) : x.subject))
            .child(div().flex_none().text_size(11).text_color(t.dim).child(ago(x.time)));
        })
        : [div().px(15).h(24).text_size(12).text_color(t.dim).child("no commits yet")];
    return [...this.commitDetail(t, f), v_flex().id("commit-inspector").flex_none().pt(4).pb(5).child(head).children(rows)];
  }

  newSpaceBar(t) {
    return h_flex().id("new-space").flex_none().px(12).py(9).gap(11).items_center().cursor_pointer().border_t(1).border_color(t.border)
      .on_size_change((size) => { this.barH = size.height; })
      .text_size(12).text_color(t.muted).hover((e) => e.bg(t.hover).text_color(t.foreground))
      .on_mouse_down("left", (e) => { this.anchor = this.anchorOf(e); })
      .on_click(() => this.chooseMachine())
      .child(div().ml(3).child("＋")).child(div().flex_1().child("New space"));
  }

  // Left: one list. The workspaces by machine; a workspace with an agent is its agent's cards (a card per topic of the agent, or
  // one for the idle agent), a workspace without one a compact row.
  left() {
    const t = theme();
    const rows = [];
    const sp = spaces();
    const list = agents();
    if (!sp.length && !list.length) return [empty(t, "◌", "No agents running", "Start an agent in a space and its card shows up here")];
    const machineOf = (a) => sp.find((s) => s.id === a.space)?.machine ?? null;
    // a space that has its default name (the shell in the home folder: "~", or the user's name on another machine) is called "<machine>: ~"
    const homeName = home().split("/").pop();
    const titled = (s) => (s.name === "~" || s.name === homeName ? `${s.machine ?? this.hostLabel}: ~` : null);
    const byMachine = new Map();
    for (const s of sp) {
      const m = s.machine ?? "This computer";
      if (!byMachine.has(m)) byMachine.set(m, []);
      byMachine.get(m).push(s);
    }
    for (const [machine, spaceList] of byMachine) {
      const remote = spaceList[0]?.machine ?? null;
      rows.push(div().flex_none().pt(8).child(this.dividerCaption(t, machine, [div().id(`ns-${machine}`).w(22).h(18).flex_none().flex().items_center().justify_center().rounded(5).text_size(14).cursor_pointer().text_color(t.dim)
        .hover((e) => e.bg(t.hover).text_color(t.foreground))
        .on_mouse_down("left", (e, cx) => { this.anchor = this.anchorOf(e); cx.stop_propagation(); })
        .on_click(this.guard(() => this.openSpaceMenu(remote, this.anchor))).child("＋")])));
      for (const s of spaceList) {
        const mine = list.filter((a) => a.space === s.id);
        if (!mine.length) {
          // a workspace without an agent: a card too, quieter, a click shows the workspace
          rows.push(this.card(t, `s-${s.id}`, { agent: null, name: titled(s) ?? s.name, branch: s.branch,
            st: { word: "", color: t.dim, bar: t.idle }, selected: s.active, dim: true, flat: true, onClick: () => run("space.select", { id: s.id }) }));
          continue;
        }
        // an agent is a card of its own: topics are not mixed in, a topic meets an agent when it is started on one
        for (const a of mine) {
          const idle = a.status === "idle" && !this.isFinished(a);
          // the state is the colour of the card: working Claude orange, error (needs you) red, recently finished green, idle grey
          const st = a.status === "working" ? { word: "working", color: "#d97757", bar: "#d97757" }
            : a.status === "blocked" ? { word: "needs you", color: t.blocked, bar: t.blocked }
            : this.isFinished(a) ? { word: "finished", color: t.ansi2 ?? t.done, bar: t.ansi2 ?? t.done } : { word: "idle", color: t.dim, bar: t.idle };
          rows.push(this.card(t, `a-${a.address}`, { agent: a, listen: this.listensTo(s), onListen: (cx) => this.toggleListen(cx, s), name: titled(s) ?? s.name, branch: s.branch, st, selected: a.focused, dim: idle, deploy: this.deployOf(s.branch), code: this.agentIdent(s, machine, remote, spaceList.indexOf(s) + 1, mine.indexOf(a) + 1, mine.indexOf(a)), finAge: this.finAge(a), monitor: this.monitorOf(a, machineOf(a)) }));
        }
      }
    }
    return rows;
  }

  // A done topic: a muted ticked box in front (a click unticks it, the topic is open again: next), the title, the ... menu.
  doneCard(t, x) {
    return h_flex().id(`dn-${x.id}`).gap(6).items_center().bg(t.surface).border(1).border_color(t.border).rounded(8).mx(8).mb(5).pl(6).pr(6).py(5)
      .hover((e) => e.bg(t.raised)).on_click(this.guard(() => this.openPage(x.id)))
      .child(div().id(`ck-dn-${x.id}`).w(24).h(24).flex_none().flex().items_center().justify_center().rounded(5).cursor_pointer()
        .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
        .on_click(this.guard((_e, cx) => this.cli(cx, ["next", x.id]))).child(checkbox(t, true, false, true)))
      .child(div().flex_1().min_w(0).truncate().text_size(12.5).text_color(t.dim).child(x.title))
      .child(this.optionsBtn(t, `op-dn-${x.id}`, () => this.topicItems(x)));
  }

  // The last three done topics: a bar that stays at the bottom of the panel under the list.
  // --- the stash: texts kept for later (written, or dictated into the field with superwhisper) --------------------------------------------------
  // A stash is a text on a queue (the shared state, so that it survives a reload). From the queue it goes to a free agent as a prompt, or becomes
  // a topic (Now, Next, Later), or is deleted. A dock at the bottom of the right panel: the head folds it, the field adds one (Enter), the list is the queue.
  stashList() {
    try { const v = shared("bhote.stash"); return Array.isArray(v) ? v : []; } catch (_e) { return []; }
  }

  setStash(list) { try { setShared("bhote.stash", list.slice(0, 200)); } catch (_e) { /* not remembered */ } }

  addStash(cx) {
    const text = this.stashIn.value().replace(/\r/g, "").trim();
    if (!text) return;
    this.setStash([...this.stashList(), { id: `${Date.now()}-${Math.floor(Math.random() * 1000)}`, text, at: Date.now() }]);
    this.stashIn.set_value("");
    cx.notify();
  }

  // What the menu of a stash offers: send it to a free agent, make a topic of it, delete it.
  stashItems(x) {
    const sp = spaces();
    const free = agents().filter((a) => a.status !== "working" && a.status !== "blocked");
    return [
      ...(free.length ? [{ type: "head", label: "Send to agent" }] : []),
      ...free.map((a) => { const s = sp.find((q) => q.id === a.space); return { type: "stash", op: "send", id: x.id, terminal: a.address, label: `${s?.name ?? a.address}${s?.machine ? ` · ${s.machine}` : ""}` }; }),
      { type: "head", label: "Make a topic" },
      { type: "stash", op: "topic", status: "now", id: x.id, label: "Now" },
      { type: "stash", op: "topic", status: "next", id: x.id, label: "Next" },
      { type: "stash", op: "topic", status: "later", id: x.id, label: "Later" },
      { type: "head", label: "Stash" },
      { type: "stash", op: "delete", id: x.id, label: "Delete", danger: true },
    ];
  }

  // A stash dragged to another place: the press starts it, moving over the list picks the place (the rows are all as high as the pitch says),
  // the release drops it there.
  dropStash(cx) {
    const d = this.drag;
    this.drag = null;
    this.dragMoved = !!d && d.from !== d.to;
    if (d && d.from !== d.to) {
      const list = this.stashList();
      const at = list.findIndex((x) => x.id === d.id);
      if (at >= 0) { const [x] = list.splice(at, 1); list.splice(Math.max(0, Math.min(list.length, d.to)), 0, x); this.setStash(list); }
    }
    cx.notify();
  }

  startEditStash(x, cx) {
    if (this.dragMoved) { this.dragMoved = false; return; }
    this.editingStash = x.id;
    this.stashEdit.set_value(x.text);
    this.stashEdit.focus();
    cx.notify();
  }

  saveStash(cx) {
    const text = this.stashEdit.value().replace(/\r/g, "").trim();
    if (text && this.editingStash) this.setStash(this.stashList().map((q) => (q.id === this.editingStash ? { ...q, text } : q)));
    this.editingStash = null;
    cx.notify();
  }

  // A small blue button with a gradient (the kit paints a gradient only on a canvas: it is a few bands from light to dark inside a rounded box).
  stashButton(t, ready) {
    // Modern: a flat rounded rectangle with a real gradient, painted as a path (`window.paint_path`; the `window` of this file is herdr's, so the
    // global one is reached through globalThis).
    const W = 54, H = 24, R = 8;
    const path = PathBuilder.fill().move_to(R, 0).line_to(W - R, 0).arc_to(R, R, 0, false, true, W, R).line_to(W, H - R).arc_to(R, R, 0, false, true, W - R, H)
      .line_to(R, H).arc_to(R, R, 0, false, true, 0, H - R).line_to(0, R).arc_to(R, R, 0, false, true, R, 0).close().build();
    return div().id("stash-send").relative().w(W).h(H).flex_none().cursor_pointer().opacity(ready ? 1 : 0.55)
      .hover((e) => e.opacity(0.9))
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .on_click(this.guard((_e, cx) => this.addStash(cx)))
      .child(globalThis.window.paint_path(path, Background.linear_gradient(180, "#ff6b6b", "#dc3545")).absolute().inset_0())
      .child(div().absolute().inset_0().flex().items_center().justify_center().text_size(11.5).font_semibold().text_color("#ffffff").child("Stash"));
  }

  stashDock(t) {
    const list = this.stashList();
    let shut = false;
    try { shut = shared("bhote.stashShut") === true; } catch (_e) { shut = false; }
    const PITCH = 50;   // a row (46) and the gap under it (4)
    const empty = this.stashIn.value().length === 0;
    return v_flex().id("stash-dock").flex_none().border_t(1).border_color(t.border).bg(t.chrome)
      .on_mouse_up("left", (_e, cx) => { if (this.drag) this.dropStash(cx); })
      .child(h_flex().id("stash-head").px(10).pt(10).pb(shut ? 10 : 6).items_center().cursor_pointer().text_size(11).font_semibold().text_color(t.dim)
        .on_click(this.guard((_e, cx) => { setShared("bhote.stashShut", !shut); if (shut) this.stashIn.focus(); cx.notify(); }))
        .child(div().flex_1().child(`STASH · ${list.length}`))
        .child(svg(shut ? "mods/bhote/chevron-up.svg" : "mods/bhote/chevron-down.svg").size(14).text_color(t.muted)))
      .when(!shut, (el) => el
        // the field: flat, with its own placeholder (italic, muted) over it while it is empty
        .child(div().px(10).pb(4)
          .child(div().relative().rounded(7).border(1).border_color(t.border).bg(t.background).px(10).py(6)
            .child(h_flex().gap(6).items_start()
              .child(div().flex_1().min_w(0).child(Textarea.new(this.stashIn).px(0).py(0).bg(t.background).text_size(12.5).text_color(t.foreground)))
              .child(this.stashButton(t, !empty))
              // dictate: the field has the focus, then superwhisper starts recording and types what it hears into it
              .child(this.ibtn(t, "stash-dictate", "mic", () => { this.stashIn.focus(); exec("open", ["superwhisper://record"]).catch(() => {}); }, { color: t.muted, hover: t.accent, w: 22 })))
            .when(empty, (e) => e.child(div().absolute().left(10).top(6).italic().text_size(12.5).text_color(t.dim).opacity(0.6).child("Stash: write, or dictate with superwhisper")))))
        .child(h_flex().px(12).pb(8).items_center().text_size(10.5).text_color(t.dim).child("Return for a new line  ·  ⌘Return stashes"))
        .child(div().h(1).w_full().bg(t.border))
        .when(list.length > 0, (e) => e.child(v_flex().id("stash-list").max_h(PITCH * 4).overflow_y_scrollbar().pt(6).pb(4)
          .on_mouse_move((ev, cx) => {
            if (!this.drag) return;
            const to = Math.max(0, Math.min(list.length - 1, Math.floor(((ev.local_position?.y ?? 0) - 6) / PITCH)));
            if (to !== this.drag.to) { this.drag.to = to; cx.notify(); }
          })
          .children(list.map((x, i) => {
            const d = this.drag;
            const moving = d && d.id === x.id;
            const before = d && d.to === i && d.to < d.from, after = d && d.to === i && d.to > d.from;
            if (this.editingStash === x.id) {
              // the stash being edited: its text in a field, saved with Enter (Shift+Enter: a new line) or the button
              return v_flex().id(`stw-${x.id}`).gap(6).bg(t.surface).border(1).border_color(t.accent).rounded(8).mx(8).mb(4).p(8)
                .child(Textarea.new(this.stashEdit).px(2).py(2).bg(t.surface).text_size(12.5).text_color(t.foreground))
                .child(h_flex().gap(8).items_center()
                  .child(div().flex_1().text_size(10.5).text_color(t.dim).child("⌘Return saves"))
                  .child(this.btn(t, `st-cancel-${x.id}`, "Cancel", (_e, cx) => { this.editingStash = null; cx.notify(); }))
                  .child(this.btn(t, `st-save-${x.id}`, "Save", (_e, cx) => this.saveStash(cx), { primary: true })));
            }
            return v_flex().id(`stw-${x.id}`)
              .when(before, (r) => r.child(div().h(2).mx(8).mb(2).rounded(1).bg(t.accent)))
              .child(h_flex().id(`st-${x.id}`).gap(8).items_center().h(46).overflow_hidden().bg(t.surface).border(1).border_color(moving ? t.accent : t.border).rounded(8).mx(8).mb(4).pl(10).pr(4).opacity(moving ? 0.6 : 1)
                .hover((r) => r.bg(t.raised))
                .on_mouse_down("left", (e, cx) => { if (!this.editingStash) { this.dragMoved = false; this.drag = { id: x.id, from: i, to: i }; cx.notify(); } })
                .on_click(this.guard((_e, cx) => this.startEditStash(x, cx)))
                .child(div().w(14).flex_none().text_size(11).text_color(t.dim).cursor_pointer().child(String(i + 1)))
                .child(div().flex_1().min_w(0).max_h(32).overflow_hidden().text_size(12).text_color(t.foreground).child(x.text.replace(/\n+/g, " ↵ ")))
                .child(this.optionsBtn(t, `op-st-${x.id}`, () => this.stashItems(x))))
              .when(after, (r) => r.child(div().h(2).mx(8).mb(2).rounded(1).bg(t.accent)));
          })))));
  }

  // The docks at the bottom of the topics: the stash, then the done topics.
  docks(t) {
    const done = this.doneDock(t);
    return [this.stashDock(t), ...(done ? [done] : [])];
  }

  doneDock(t) {
    const all = this.topics.filter((x) => x.group === "done");
    if (!all.length) return null;
    let shut = true;   // folded by default
    try { shut = shared("bhote.doneShut") !== false; } catch (_e) { shut = true; }
    const done = all.slice(0, 3);
    return v_flex().flex_none().pb(shut ? 0 : 4).border_t(1).border_color(t.border).bg(t.chrome)
      .child(h_flex().id("done-head").px(10).pt(10).pb(shut ? 10 : 4).items_center().cursor_pointer().text_size(11).font_semibold().text_color(t.dim)
        .on_click(this.guard((_e, cx) => { setShared("bhote.doneShut", !shut); cx.notify(); }))
        .child(div().flex_1().child(`DONE · ${all.length}`))
        .child(svg(shut ? "mods/bhote/chevron-up.svg" : "mods/bhote/chevron-down.svg").size(14).text_color(t.muted)))
      .children(shut ? [] : done.map((x) => this.doneCard(t, x)));
  }

  // The state of a topic by itself (no agent): its word and colour.
  plainState(t, x) {
    const g = x.group;
    if (g === "now") return { word: "now", color: t.working, icon: "now" };
    if (g === "review") return { word: "review", color: t.ansi4, icon: "review" };
    if (g === "testing") return { word: "testing", color: t.ansi6, icon: "testing" };
    if (g === "deploying") return { word: "waiting for deployment", color: t.ansi3, icon: "deploying" };
    if (g === "finished") return { word: "finished", color: t.done, icon: "ok" };
    if (g === "next") return { word: "next", color: t.ansi3, icon: "next" };
    if (g === "waiting") return { word: x.waiting_for ? `waiting for ${x.waiting_for}` : "waiting", color: t.ansi3, icon: "waiting" };
    if (g === "later") return { word: "parked", color: t.dim, icon: "later" };
    return { word: "done", color: t.done, icon: "done" };
  }

  // A topic looks like a to-do: the icon of its state, a small title on one line, the ... menu; what it waits for under the title.
  line(t, x, cx, sub) {
    const st = this.plainState(t, x);
    const queued = this.queueOf(x.id);
    const extra = queued ? `queued on ${queued.name}` : x.group === "waiting" && x.waiting_for ? `waiting for ${x.waiting_for}` : "";
    const picked = this.sel().topics.has(x.id);
    return h_flex().id(`t-${x.id}`).gap(6).items_center().bg(t.surface).border(1).border_color(picked ? t.accent : t.border).rounded(8).mx(8).mb(5).pl(6).pr(6).py(5)
      .hover((s) => s.bg(t.raised)).on_click(this.guard(() => this.openPage(x.id)))
      .child(this.check(t, `ck-${x.id}`, "topics", x.id, picked))
      .child(v_flex().flex_1().min_w(0).gap(1)
        .child(div().truncate().text_size(12.5).text_color(t.foreground).child(x.title))
        .when(!!(sub || extra), (el) => el.child(div().truncate().text_size(11).text_color(t.muted).child(sub || extra))))
      .child(this.icon(t, st.icon, st.color, 16))
      .child(this.optionsBtn(t, `op-t-${x.id}`, () => this.topicItems(x)));
  }

  // Open and recently merged pull requests of the projects' repositories (gh), and the runs of branches that have no pull
  // request yet. Merged ones get the deployments that started after the merge (bhoted's deploys.list, by project and time).
  loadPRs(cx) {
    if (this.prLoading) return;
    this.prLoading = true;
    cx.spawn(async (cx) => {
      await this.cfgPromise;
      const out = [];
      const builds = [];
      const baseCache = new Map();
      // the runs of the base branch (GitHub Actions), once per branch: was a workflow broken there, and is it green again?
      const baseRuns = async (slug, base) => {
        const k = `${slug}|${base}`;
        if (!baseCache.has(k)) {
          let runs = [];
          try {
            const r = await exec("gh", ["run", "list", "-R", slug, "--branch", base, "--limit", "30", "--json", "workflowName,conclusion,createdAt"], { timeout: 40000 });
            runs = JSON.parse(r.stdout).map((x) => ({ wf: x.workflowName, ok: x.conclusion === "success", bad: x.conclusion === "failure", t: secOf(x.createdAt) }));
          } catch (_e) { /* no runs known */ }
          baseCache.set(k, runs);
        }
        return baseCache.get(k);
      };
      try {
        const cardStates = new Set(["review", "now", "testing", "finished"]);
        const since = Date.now() / 1000 - 14 * 86400;
        for (const p of this.projects) {
          for (const repo of p.repos) {
            const m = /^github\.com\/([^/]+\/[^/]+)$/i.exec(repo);
            if (!m) continue;
            const slug = m[1];
            let list = [];
            try {
              const r = await exec("gh", ["pr", "list", "-R", slug, "--author", "@me", "--state", "all", "--limit", "20", "--json", PR_FIELDS], { timeout: 40000 });
              list = JSON.parse(r.stdout);
            } catch (_e) { /* gh missing, not signed in or offline: this repo shows nothing */ }
            const heads = new Set();
            for (const q of list) {
              const merged = q.state === "MERGED" ? Math.floor(Date.parse(q.mergedAt) / 1000) : 0;
              if (q.state === "CLOSED" || (merged && merged < since)) continue;
              heads.add(q.headRefName);
              const checks = (q.statusCheckRollup ?? []).map(checkOf);
              this.observeChecks(p.title, checks);
              // A red check that the base branch has fixed since (by another pull request) is not shown as failed: the same workflow was broken on
              // the base branch when this one ran (a failed run up to ten minutes after it, not older than three days) and a later run there passed.
              if (!merged && q.baseRefName && checks.some((c) => c.state === "failed" && c.workflow)) {
                const runs = await baseRuns(slug, q.baseRefName);
                for (const c of checks) {
                  if (c.state !== "failed" || !c.workflow) continue;
                  const when = c.ended || c.started || 0;
                  const wf = runs.filter((r) => r.wf === c.workflow);
                  const broke = wf.filter((r) => r.bad && r.t <= when + 600 && r.t >= when - 3 * 86400).sort((a, b) => b.t - a.t)[0];
                  if (broke && wf.some((r) => r.ok && r.t > broke.t)) { c.state = "solved"; c.solvedOn = q.baseRefName; }
                }
              }
              out.push({ kind: "pr", project: p.title, slug, number: q.number, title: q.title, branch: q.headRefName, base: q.baseRefName, url: q.url, draft: !!q.isDraft, merged, updated: Math.floor(Date.parse(q.updatedAt) / 1000) || 0, review: q.reviewDecision || "", checks });
            }
            const deploys = !this.deployOff.has(p.title);   // a project that is not deployed through CI has no builds and pipelines to show here
            // the builds that run now on any open pull request (also those of other people): they are what a deployment hangs on
            if (deploys) try {
              const r = await exec("gh", ["pr", "list", "-R", slug, "--state", "open", "--limit", "40", "--json", "number,title,headRefName,url,author,statusCheckRollup"], { timeout: 40000 });
              for (const q of JSON.parse(r.stdout)) {
                const cks = (q.statusCheckRollup ?? []).map(checkOf);
                const live = cks.filter((c) => c.state === "running");
                if (!live.length) continue;
                this.observeChecks(p.title, cks);
                const finished = cks.length - live.length;
                for (const c of live) {
                  builds.push({ time: c.started || Math.floor(Date.now() / 1000), project: p.title, label: c.name.replace(/^ci\/circleci:\s*/i, ""), state: "running", done: 0, total: 0,   // (its own progress by its usual duration: the share of finished checks is the pull request's, it has it in its tile)
                    branch: q.headRefName, title: `${q.author?.login ?? ""} · #${q.number} ${q.title}`.replace(/^ · /, ""), url: c.url || q.url, histKey: c.key });
                }
              }
            } catch (_e) { /* no builds known */ }
            // the pipelines on the deploy branches (main): what a deployment hangs on. GitHub has their state: the statuses (CircleCI) and the check
            // runs (Actions) of the newest commit. What runs is shown, and what failed in the last three hours.
            if (deploys) for (const b of (p.deployBranches.length ? p.deployBranches : ["main"])) {
              try {
                const sh = async (path, jq) => JSON.parse((await exec("gh", ["api", path, "--jq", jq], { timeout: 40000 })).stdout);
                const [cm, sts, runs] = await Promise.all([
                  sh(`repos/${slug}/commits/${b}`, '{sha: .sha, msg: (.commit.message | split("\n") | .[0]), author: .author.login, date: .commit.committer.date, url: .html_url}'),
                  sh(`repos/${slug}/commits/${b}/status`, '[.statuses[] | {context, state, target_url, created_at, updated_at}]').catch(() => []),
                  sh(`repos/${slug}/commits/${b}/check-runs`, '[.check_runs[] | {name, status, conclusion, started_at, html_url}]').catch(() => []),
                ]);
                const cks = [
                  ...sts.map((x) => checkOf({ __typename: "StatusContext", context: x.context, state: String(x.state).toUpperCase(), targetUrl: x.target_url, startedAt: x.created_at, completedAt: x.state === "pending" ? "" : x.updated_at })),
                  ...runs.map((x) => checkOf({ __typename: "CheckRun", name: x.name, status: String(x.status).toUpperCase(), conclusion: x.conclusion, detailsUrl: x.html_url, startedAt: x.started_at })),
                ];
                this.observeChecks(p.title, cks);
                const age = Date.now() / 1000 - secOf(cm.date);
                const shown = cks.filter((c) => c.state === "running" || (c.state === "failed" && age < 3 * 3600));
                const finished = cks.filter((c) => c.state !== "running").length;
                const title = `${cm.author ?? ""} · ${cm.msg ?? ""}`.replace(/^ · /, "");
                for (const c of shown) {
                  builds.push({ time: c.started || secOf(cm.date), project: p.title, label: c.name.replace(/^ci\/circleci:\s*/i, ""), state: c.state, done: finished, total: cks.length,
                    branch: b, title, url: c.url || cm.url, histKey: c.key });
                }
                // Nothing runs and nothing failed: the newest pipeline of the branch is the last deployment. One row per deploy job (deploy-ecs-backend),
                // or one row named after the environment when the jobs are not known; the time is when it finished.
                if (!cks.some((c) => c.state === "running") && !shown.length && cks.some((c) => c.state === "ok")) {
                  const doneAt = Math.max(...cks.map((c) => c.ended || c.started || 0), secOf(cm.date));
                  const jobs = cks.filter((c) => c.state === "ok" && p.deployJobs.some((j) => c.name.toLowerCase().includes(j.toLowerCase())));
                  const rows = jobs.length ? jobs.map((c) => c.name.replace(/^ci\/circleci:\s*/i, "")) : [p.stages?.[Math.max(0, p.deployBranches.indexOf(b))] ?? "Deploy"];
                  for (const label of rows) builds.push({ time: doneAt, project: p.title, label, state: "ok", done: cks.length, total: cks.length, branch: b, title, url: cm.url, histKey: null });
                }
                // The deploy jobs of the project that wait for their build (CircleCI shows them with an hourglass; GitHub hears of a job only when it
                // starts): a deploy job (deploy-ecs-backend) waits while the build of its part (build-backend) runs and the job has no status yet.
                const live = cks.filter((c) => c.state === "running");
                for (const job of p.deployJobs) {
                  if (!live.length || cks.some((c) => c.name.toLowerCase().includes(job.toLowerCase()))) continue;
                  const part = job.split("-").pop().toLowerCase();
                  if (!live.some((c) => c.name.toLowerCase().replace(/^ci\/circleci:\s*/, "").split("-").pop() === part)) continue;
                  builds.push({ time: secOf(cm.date), project: p.title, label: job, state: "waiting", done: 0, total: 0, branch: b, title, url: cm.url, histKey: null });
                }
              } catch (_e) { /* no pipelines known on this branch */ }
            }
            // branches of topics that work on this project and have no pull request yet
            const branches = [...new Set(this.topics.filter((x) => cardStates.has(x.group) && x.branch && !x.dismissed && (x.project === p.id)).map((x) => x.branch))].filter((b) => !heads.has(b) && b !== "main" && b !== "develop");
            for (const b of branches.slice(0, 6)) {
              let runs = [];
              try {
                const r = await exec("gh", ["run", "list", "-R", slug, "--branch", b, "--limit", "8", "--json", "name,workflowName,status,conclusion,url,createdAt"], { timeout: 40000 });
                runs = JSON.parse(r.stdout);
              } catch (_e) { /* no runs known */ }
              const seen = new Set();
              const checks = runs.map(runOf).filter((c) => (seen.has(c.name) ? false : seen.add(c.name)));   // the newest run of each workflow
              this.observeChecks(p.title, checks);
              out.push({ kind: "branch", project: p.title, slug, number: 0, title: b, branch: b, base: "", url: `https://github.com/${slug}/tree/${b}`, draft: false, merged: 0, updated: 0, review: "", checks });
            }
          }
        }
        this.prs = out;
        this.builds = builds;
        this.prAt = Date.now();
      } catch (e) {
        this.error = String(e?.message ?? e);
      }
      this.prLoading = false;
      cx.notify();
    });
  }

  // What the Deployments tab shows: the deployments (bhoted) and the builds that run on open pull requests (gh), newest first.
  deploysAll() {
    return [...this.deploys, ...(this.builds ?? [])].filter((x) => !this.deployOff.has(this.projectOf(x.project))).sort((a, b) => b.time - a.time);
  }

  // What a pipeline of a deploy branch is about: the pull request behind it ("#1784 fix: orphaned renewal plan ..."), found by the number in the
  // merge message; for one we do not know the number and the branch of the merge; else the commit title. `author` is who pushed it.
  whatOf(x) {
    const raw = x.title ?? "";
    const i = raw.indexOf(" · ");
    const author = i >= 0 ? raw.slice(0, i) : "";
    const msg = i >= 0 ? raw.slice(i + 3) : raw;
    const m = /#(\d+)/.exec(msg);
    if (!m) return { text: msg, author };
    const pr = this.prs.find((q) => String(q.number) === m[1] && (!x.project || q.project === x.project));
    if (pr) return { text: `#${pr.number} ${pr.title}`, author };
    const b = /from [^/]+\/(.+)$/.exec(msg);
    return { text: b ? `#${m[1]} ${b[1]}` : msg, author };
  }

  // The deployments of a project that started after a pull request was merged: the first one of each environment.
  deploysAfter(pr) {
    const seen = new Map();
    for (const x of [...this.deploys].sort((a, b) => a.time - b.time)) {
      if (x.project !== pr.project || x.time < pr.merged || seen.has(x.label)) continue;
      seen.set(x.label, x);
    }
    return [...seen.values()];
  }

  // The open topic that is about this pull request ("PR #33: ..."), if any.
  topicOf(pr) {
    if (!(pr.number > 0)) return null;
    return this.topics.find((x) => x.group !== "done" && !x.dismissed && new RegExp(`\\bPR #${pr.number}\\b`).test(x.title)) ?? null;
  }

  // A topic to fix a pull request's failing checks: next in line (not started by itself), the failing checks as its description.
  createFix(cx, pr) {
    const failed = pr.checks.filter((c) => c.state === "failed").map((c) => c.name);
    const desc = `${failed.join(", ")} fail on ${pr.branch}`.slice(0, 140);
    this.cli(cx, ["add", `PR #${pr.number}: fix the failing checks`, "-d", desc, "-s", "next"]);
  }

  // One pull request (or branch) as a card: its title, review state, the counts of its checks, what fails or runs, and
  // for a merged one the deployments after the merge.
  // Where a pull request stands, in the order the project works: tests (the checks), merge, then the environments of the project (its
  // deploy branches: Staging, Production, or just Deploy); a project that is switched off for deployments ends at the merge.
  stagesOf(pr) {
    const proj = this.projects.find((q) => q.title === pr.project);
    const has = (st) => pr.checks.some((c) => c.state === st);
    const tests = has("failed") ? "failed" : has("running") ? "running" : has("ok") ? "ok" : "none";
    const builds = this.builds.filter((b) => b.title.includes(`#${pr.number}`) && b.project === pr.project);
    return runbookStages(proj, { hasPR: pr.kind === "pr", tests, merged: !!pr.merged, target: pr.base, off: this.deployOff.has(pr.project), builds });
  }

  // The path as a strip: an icon and a name for each stage, joined by short lines; the stage the work is at stands out. The strip has a border
  // and a ground a little different from the card, so that it reads as the way the work goes.
  pathRow(t, st) {
    const at = st.findIndex((x) => x.state !== "ok");
    const iconOf = (x) => x.state === "ok" ? ["ok", t.ansi2 ?? t.done] : x.state === "failed" ? ["failed", t.blocked] : x.state === "running" ? ["running", t.working]
      : x.state === "approval" ? ["approval", t.ansi3] : x.state === "ready" ? ["next", t.accent] : ["waiting", t.dim];
    // a stage that is over says so in the past: a green tick and Tested, Merged ... ; the target of the merge and the note in brackets (Docker, Capistrano) belong to the name
    const past = { Commit: "Committed", PR: "PR opened", Tests: "Tested", Merge: "Merged", Build: "Built", Approval: "Approved", Release: "Released", Deploy: "Deployed" };
    const labelOf = (x) => {
      const base = x.name.split(" (")[0], note = x.name.includes(" (") ? x.name.slice(x.name.indexOf(" (")) : "";
      const w = x.state === "ok" ? past[base] ?? base : base;
      return `${w}${base === "Merge" && x.target ? ` → ${x.target}` : ""}${x.state === "ok" ? "" : note}`;
    };
    // a long path (more than five stages) names the stage the work is at and the next one, and only shows the icon of the others
    const many = st.length > 5;
    const named = (i) => !many || i === at || i === at + 1 || (at < 0 && i === st.length - 1);
    return h_flex().w_full().gap(6).items_center().px(11).py(4).my(4).rounded(6).bg(t.chrome).border(1).border_color(t.border).text_size(11)
      .children(st.flatMap((x, i) => {
        const [name, color] = iconOf(x);
        const here = i === at;
        const chip = h_flex().gap(4).items_center().flex_none().text_color(here ? t.foreground : t.dim)
          .child(name === "running" ? this.stateIcon(t, "running", color, 14) : this.icon(t, name, color, 14)).when(named(i), (c) => c.child(div().child(labelOf(x))));
        return i === 0 ? [chip] : [div().flex_1().min_w(4).h(1).bg(t.border), chip];
      }));
  }

  stageRow(t, pr) {
    return this.pathRow(t, this.stagesOf(pr));
  }

  // The path of a run (a test run, a build or a deployment): where its work stands. A pipeline of a deploy branch has passed tests and merge and
  // is at its environment (its place in the project's deploy branches: Staging before Production); a run on a branch is at the tests.
  runStages(x, kind) {
    const proj = this.projects.find((q) => q.title === this.projectOf(x.project));
    if (!proj) return null;
    const now = x.state === "running" || x.state === "queued" ? "running" : x.state === "approval" ? "approval" : x.state === "failed" ? "failed" : x.state === "ok" ? "ok" : "waiting";
    const bi = kind === "d" ? proj.deployBranches.indexOf(x.branch) : -1;
    const post = postStages(proj);
    if (bi >= 0) {   // a run on a deploy branch: its environment (by name when the post stages carry it), else the first stage that is built or deployed
      const byName = proj.stages?.[bi] ? post.findIndex((l) => l.split(" (")[0] === proj.stages[bi]) : -1;
      let at = byName >= 0 ? byName : Math.min(bi, Math.max(0, post.length - 1));
      const hold = now === "approval" ? post.findIndex((l) => l.split(" (")[0] === "Approval") : -1;   // (a pipeline that waits for approval is at the Approval stage)
      if (hold >= 0) at = hold;
      return runbookStages(proj, { hasPR: true, tests: "ok", merged: true, target: proj.deployBranches[bi], at: post.length ? at : -1, now });
    }
    return runbookStages(proj, { hasPR: true, tests: now, merged: false, target: proj.deployBranches[0] ?? "" });
  }

  prCard(t, id, pr, o = {}) {
    const by = (st) => pr.checks.filter((c) => c.state === st);
    const failed = by("failed"), running = by("running"), ok = by("ok");
    const color = pr.merged ? t.ansi5 : failed.length ? t.blocked : running.length ? t.working : ok.length ? t.done : t.dim;
    const chip = pr.merged ? ["merged", t.ansi5] : pr.draft ? ["draft", t.dim]
      : pr.review === "APPROVED" ? ["approved", t.done] : pr.review === "CHANGES_REQUESTED" ? ["changes requested", t.blocked]
      : pr.review === "REVIEW_REQUIRED" ? ["review needed", t.muted] : pr.kind === "branch" ? ["no PR yet", t.dim] : null;
    const deploys = pr.merged ? this.deploysAfter(pr) : [];
    const dcolor = (x) => (x.state === "ok" ? t.done : x.state === "failed" ? t.blocked : x.state === "approval" ? t.ansi3 : x.state === "running" ? t.working : t.dim);
    const chipIcon = pr.merged ? "merged" : pr.draft ? "draft" : pr.review === "APPROVED" ? "ok" : pr.review === "CHANGES_REQUESTED" ? "failed" : pr.review === "REVIEW_REQUIRED" ? "review" : pr.kind === "branch" ? "branch" : null;
    const items = () => {
      const topic = this.topicOf(pr);
      const failedNames = failed.map((c) => c.name);
      return [
        { type: "url", url: pr.url, label: pr.kind === "branch" ? "Open branch in GitHub" : "Open in GitHub" },
        ...(topic ? this.topicItems(topic) : []),
        ...(!topic && pr.number > 0 && failedNames.length ? [{ type: "create", title: `PR #${pr.number}: fix the failing checks`, desc: `${failedNames.join(", ")} fail on ${pr.branch}`.slice(0, 140), label: "Create topic for the failing checks" }] : []),
      ];
    };
    return h_flex().id(id).bg(t.surface).border(1).border_color(failed.length && !pr.merged ? "#a8162e" : t.border).rounded(8).mx(8).mb(6)
      .hover((x) => x.bg(t.raised)).on_click(this.guard(() => this.open(pr.url)))
      .child(div().w(3).flex_none().rounded(2).my(6).ml(6).bg(color))
      .child(v_flex().flex_1().min_w(0).gap(3).px(10).py(8)
        .child(h_flex().gap(7).items_center()
          .when(!!chipIcon, (el) => el.child(this.icon(t, chipIcon, chip ? chip[1] : t.muted, 15)))
          .when(pr.number > 0, (el) => el.child(div().flex_none().text_color(t.dim).child(`#${pr.number}`)))
          .child(div().flex_1().min_w(0).truncate().font_semibold().text_color(t.foreground).child(pr.title))
          .child(this.optionsBtn(t, `op-${id}`, items)))
        .child(div().truncate().text_size(11).text_color(t.muted).child(`⎇ ${pr.branch} · ${pr.project}`))
        .when(pr.kind === "pr" && !pr.merged, (el) => el.child(this.stageRow(t, pr)))
        .when(pr.checks.length > 0 && running.length > 0, (el) => {
          const e = this.checkEstimate(pr.checks);
          return el.child(h_flex().gap(8).items_center().text_size(11).text_color(t.working)
            .child(progress(t, e.frac ?? 0, failed.length ? t.blocked : t.working))
            .child(div().child(`${e.finished}/${e.total} checks`))
            .when(e.remain != null, (r) => r.child(div().text_color(t.muted).child(left(e.remain)))));
        })
        .when(pr.checks.length > 0, (el) => el.child(h_flex().gap(12).text_size(12)
          .child(h_flex().gap(4).items_center().text_color(t.done).child(this.icon(t, "ok", t.done, 14)).child(String(ok.length)))
          .when(failed.length > 0, (e) => e.child(h_flex().gap(4).items_center().text_color(t.blocked).child(this.icon(t, "failed", t.blocked, 14)).child(String(failed.length))))
          .when(running.length > 0, (e) => e.child(h_flex().gap(4).items_center().text_color(t.working).child(this.stateIcon(t, "running", t.working, 14)).child(String(running.length))))))
        .when(by("solved").length > 0, (el) => el.child(div().text_size(11).text_color(t.dim).child(`${by("solved").length} failed check${by("solved").length === 1 ? "" : "s"} fixed on ${by("solved")[0].solvedOn}`)))
        .when(pr.checks.length === 0, (el) => el.child(div().text_size(11).text_color(t.dim).child("no checks seen")))
        .children(failed.slice(0, 3).map((c) => h_flex().gap(6).pl(2).items_center().text_size(11).text_color(t.blocked).child(this.icon(t, "failed", t.blocked, 14)).child(div().flex_1().min_w(0).truncate().child(c.name))))
        .children(running.slice(0, 2).map((c) => h_flex().gap(6).pl(2).items_center().text_size(11).text_color(t.working).child(this.stateIcon(t, "running", t.working, 14)).child(div().flex_1().min_w(0).truncate().child(c.name))))
        .when(pr.merged > 0, (el) => el.child(v_flex().gap(2).pt(2)
          .child(div().text_size(11).text_color(t.dim).child(deploys.length ? `deployments after the merge · ${ago(pr.merged)}` : `merged ${ago(pr.merged)} · no deployment seen`))
          .children(deploys.map((x) => h_flex().gap(6).text_size(12)
            .child(this.stateIcon(t, x.state, dcolor(x), 13))
            .child(div().flex_1().truncate().text_color(t.foreground).child(x.label))
            .child(div().flex_none().text_size(11).text_color(dcolor(x)).child(x.state === "ok" ? "deployed" : x.state)))))));
  }

  // The project the PRs tab is filtered to (null: all), kept in the shared state so that the menu popup can set it.
  prFilter() {
    try { const v = shared("bhote.prFilter"); return new Set(Array.isArray(v) ? v : v ? [v] : []); } catch (_e) { return new Set(); }
  }

  prProjects() {
    return [...new Set(this.prs.map((x) => x.project))].sort((a, b) => a.localeCompare(b));
  }

  // The filter button at the right of the tab bar: the project, a chevron; it opens the list of projects.
  prFilterButton(t) {
    const f = this.prFilter();
    const label = f.size === 0 ? "All projects" : f.size === 1 ? [...f][0] : `${f.size} projects`;
    return div().id("pr-filter").flex_none().flex().items_center().gap(5).px(9).py(4).rounded(6).text_size(12).cursor_pointer()
      .bg(f.size ? t.selected : t.raised).border(1).border_color(f.size ? t.accent : t.border).text_color(f.size ? t.foreground : t.muted)
      .hover((e) => e.bg(t.hover))
      .on_mouse_down("left", (e) => { this.anchor = this.anchorOf(e); })
      .on_click(this.guard(() => {
        const cur = this.prFilter();
        this.openMenu([{ type: "head", label: "Filter by project" },
          { type: "prtoggle", value: "", check: cur.size === 0, label: "All projects" },
          ...this.prProjects().map((p) => ({ type: "prtoggle", value: p, check: cur.has(p), label: p }))], this.anchor);
      }))
      .child(div().child(label))
      .child(svg("mods/bhote/chevron-down.svg").size(12).text_color(t.dim));
  }

  // A merged pull request as a slim row: the merge icon, number, title, when it was merged, the state of its deployments (when there are
  // some), the ... menu. Everything else is on the card of an open one.
  prRow(t, id, pr) {
    const deps = [...this.deploysAfter(pr), ...this.builds.filter((b) => b.title.includes(`#${pr.number}`) && b.project === pr.project)];
    const dstate = deps.some((x) => x.state === "failed") ? "failed" : deps.some((x) => x.state === "running") ? "running"
      : deps.some((x) => x.state === "approval") ? "approval" : deps.length ? "ok" : null;
    const dcolor = dstate === "failed" ? t.blocked : dstate === "running" ? t.working : dstate === "approval" ? t.ansi3 : t.done;
    const items = () => {
      const topic = this.topicOf(pr);
      return [{ type: "url", url: pr.url, label: "Open in GitHub" }, ...(topic ? this.topicItems(topic) : [])];
    };
    return h_flex().id(id).gap(8).items_center().bg(t.surface).border(1).border_color(t.border).rounded(8).mx(8).mb(4).pl(10).pr(6).py(5)
      .hover((x) => x.bg(t.raised)).on_click(this.guard(() => this.open(pr.url)))
      .child(this.icon(t, "merged", t.ansi5, 15))
      .child(div().flex_none().text_size(12).text_color(t.dim).child(`#${pr.number}`))
      .child(div().flex_1().min_w(0).truncate().text_size(12.5).text_color(t.muted).child(pr.title))
      .child(div().flex_none().text_size(11).text_color(t.dim).child(ago(pr.merged)))
      .when(!!dstate, (el) => el.child(this.stateIcon(t, dstate, dcolor, 15)))
      .child(this.optionsBtn(t, `op-${id}`, items));
  }

  prsBody(t) {
    const rows = [];
    const f = this.prFilter();
    const all = f.size ? this.prs.filter((x) => f.has(x.project)) : this.prs;
    const open = all.filter((x) => x.kind === "pr" && !x.merged);
    const branches = all.filter((x) => x.kind === "branch");
    const merged = all.filter((x) => x.merged).sort((a, b) => b.merged - a.merged);
    const sections = [["Open pull requests", open, false], ["Branches without a pull request", branches, false], ["Merged, last 14 days", merged, true]];
    for (const [label, list, slim] of sections) {
      if (!list.length) continue;
      rows.push(this.head(t, `${label} · ${list.length}`));
      // under each group the projects, the one with the newest activity first; in a project the newest first
      const failingIn = (xs) => xs.some((x) => x.checks.some((c) => c.state === "failed"));
      const byProject = new Map();
      const stamp = (x) => x.merged || x.updated || 0;
      for (const x of [...list].sort((a, b) => stamp(b) - stamp(a))) { if (!byProject.has(x.project)) byProject.set(x.project, []); byProject.get(x.project).push(x); }
      const newest = (xs) => Math.max(0, ...xs.map(stamp));
      const projects = [...byProject.entries()].sort((a, b) => newest(b[1]) - newest(a[1]) || a[0].localeCompare(b[0]));
      let shown = 0;
      const cap = slim ? 15 : 12;
      for (const [project, xs] of projects) {
        if (shown >= cap) break;
        rows.push(h_flex().px(12).pt(8).pb(3).gap(6).items_center().text_size(11).text_color(t.muted)
          .child(div().w(6).h(6).rounded(3).flex_none().bg(failingIn(xs) ? t.blocked : t.dim))
          .child(div().flex_1().min_w(0).truncate().font_semibold().child(project))
          .child(div().flex_none().text_color(t.dim).child(String(xs.length))));
        for (const x of xs) {
          if (shown >= cap) break;
          rows.push(slim ? this.prRow(t, `pr-${label.length}-${shown}`, x) : this.prCard(t, `pr-${label.length}-${shown}`, x));
          shown += 1;
        }
      }
    }
    if (!rows.length) rows.push(this.prLoading || !this.prAt ? empty(t, "", "Loading pull requests", "Asking GitHub for your open pull requests", true) : empty(t, "⑂", f.size ? `Nothing for ${[...f].join(", ")}` : "No open pull requests", f.size ? "Pick All projects in the filter to see the others" : "Your pull requests and the branches you work on show up here with their checks"));
    return rows;
  }

  // The tab bar of the right panel: Topics, Deployments, Tests; a monitor's tab shows what runs (orange) and what failed (red).
  tabBar(t, cx) {
    const prList = this.prs.filter((x) => !x.merged).map((x) => ({ state: x.checks.some((c) => c.state === "failed") ? "failed" : x.checks.some((c) => c.state === "running") ? "running" : "ok" }));
    const tabs = [["topics", "Topics", null], ["prs", "PRs", prList], ["deploys", "Deploys", this.deploysAll()], ["tests", "Tests", this.testsShown().shown], ["servers", "Servers", null]];
    return h_flex().flex_none().gap(2).px(8).pt(8).pb(6).items_center().border_b(1).border_color(t.border)
      .child(div().id("tab-home").w(28).h(26).flex_none().flex().items_center().justify_center().rounded(6).cursor_pointer()
        .bg(t.chrome).hover((x) => x.bg(t.hover)).on_click((_e, cx) => this.setTab(cx, "home"))
        .child(svg("mods/bhote/grid.svg").size(15).text_color(t.muted)))
      .children(tabs.map(([id, label, list]) => {
        const on = this.tab === id;
        const running = list ? list.filter((x) => x.state === "running" || x.state === "queued" || x.state === "approval").length : 0;
        const failed = list ? list.filter((x) => x.state === "failed").length : 0;
        return h_flex().id(`tab-${id}`).gap(4).px(7).py(4).rounded(6).items_center().text_size(12)
          .bg(on ? t.selected : t.chrome).text_color(on ? t.foreground : t.muted).hover((x) => x.bg(on ? t.selected : t.hover))
          .on_click((_e, cx) => this.setTab(cx, id))
          .child(label)
          .when(running > 0, (el) => el.child(div().text_size(11).text_color(t.working).child(String(running))))
          .when(failed > 0, (el) => el.child(div().px(4).rounded(8).text_size(10).bg("#a8162e").text_color("#ffffff").child(String(failed))));
      }))
      .when(this.tab === "prs", (el) => el.child(div().flex_1()).child(this.prFilterButton(t)));
  }

  // The project a run belongs to: runs are named after worktrees ("IMS-wt1", "IMS (wt2)"); they belong to the project of that name.
  projectOf(name) {
    const titles = new Map(this.projects.map((p) => [p.title.toLowerCase(), p.title]));
    const base = (name ?? "").replace(/[\s-]*\(?wt\d+\)?$/i, "").trim() || name;
    return titles.get((base ?? "").toLowerCase()) ?? base;
  }

  // The runs of a monitor grouped by project: the project with the newest run first; each group says what runs and what failed
  // and shows its ten newest runs.
  monitorBody(t, rows, kind, prefix) {
    const projectOf = (name) => this.projectOf(name);
    const groups = new Map();
    for (const x of rows) {
      const g = projectOf(x.project);
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g).push(x);
    }
    const out = [];
    const ordered = [...groups.entries()].sort((a, b) => Math.max(...b[1].map((x) => x.time)) - Math.max(...a[1].map((x) => x.time)));
    for (const [project, all] of ordered) {
      // at most two passed runs per project (the newest ones); every running and failed run stays
      let passed = 0;
      const list = all.filter((x) => (x.state !== "ok" && x.state !== "canceled" && x.state !== "fixed") || ++passed <= 2);
      const hidden = all.length - list.length;
      const running = list.filter((x) => x.state === "running" || x.state === "queued" || x.state === "approval").length;
      const failed = list.filter((x) => x.state === "failed").length;
      out.push(this.head(t, `${project} · ${list.length}`, h_flex().gap(6).items_center()
        .when(hidden > 0, (el) => el.child(div().text_color(t.dim).child(`+${hidden} more`)))
        .when(running > 0, (el) => el.child(div().text_color(t.working).child(`${running} running`)))
        .when(failed > 0, (el) => el.child(div().px(5).rounded(8).bg("#a8162e").text_color("#ffffff").child(`✗ ${failed}`))), true));
      list.slice(0, 10).forEach((x, i) => out.push(this.monitorCard(t, `${prefix}-${project}-${i}`, x, kind, false)));
    }
    return out;
  }

  // --- the servers: the IMS dev systems (main, wt1, wt2, wt3) of this computer and of the other machines ----------------------------------
  // The state comes from servers.js (this folder; it is copied to the other machines), the actions are the dev shell's own commands:
  // ims-<slot>-start (starts, or restarts a system that runs), ims-<slot>-stop, ims-start and ims-stop (all of the machine), and
  // ims-tunnels-start / ims-tunnels-stop (the tunnel to the other machine, set up in the main checkout's server/.env).
  srvKey(name) { return name ?? "local"; }

  srvMachines() {
    const out = [{ name: null, label: this.hostLabel }];
    try { for (const m of machines()) if (m.name && m.layout !== "unavailable") out.push({ name: m.name, label: this.machineLabel(m.name) }); } catch (_e) { /* this computer only */ }
    return out;
  }

  srvExec(name, cmd, args, timeout) {
    return name == null ? exec(cmd, args, { timeout: timeout ?? 60000 }) : exec("hc", ["exec", "--on", name, cmd, ...args], { timeout: timeout ?? 60000 });
  }

  // Once per machine: its IMS checkout and its node (from the dev shell, so the node the shell uses), and the script copied over.
  async srvResolve(name) {
    const key = this.srvKey(name);
    const r = await this.srvExec(name, "zsh", ["-ic", '_ims_node24 >/dev/null 2>&1; print -r -- "main=$_ims_main"; print -r -- "node=$(whence -p node)"; print -r -- "home=$HOME"'], 30000);
    const get = (k) => ((new RegExp(`^${k}=(.*)$`, "m").exec(r.stdout) ?? [])[1] ?? "").trim();
    const info = { main: get("main"), node: get("node"), home: get("home") };
    if (!info.main || !info.node) { this.srvInfo[key] = { none: true }; return; }   // no IMS here
    const mine = `${home()}/.config/herdr-app/mods/bhote/servers.js`;
    info.script = name == null ? mine : `${info.home}/.cache/bhote/servers.js`;
    if (name != null) await exec("sh", ["-c", `hc exec --on '${name}' sh -c 'mkdir -p "$HOME/.cache/bhote"' && hc file write --on '${name}' '${info.script}' < '${mine}'`], { timeout: 30000 });
    this.srvInfo[key] = info;
    if (name == null) { try { setShared("bhote.imsMain", info.main); } catch (_e) { /* the settings ask the shell themselves */ } }
  }

  loadServers(cx) {
    if (this.srvLoading) return;
    this.srvLoading = true;
    this.srvAt = Date.now();
    cx.spawn(async (cx) => {
      try { await this.loadSvcState(); await this.checkSvcFailures(); cx.notify(); } catch (_e) { /* the last state stays (the services do not wait for the slow IMS look) */ }
      for (const m of this.srvMachines()) {
        const key = this.srvKey(m.name);
        try {
          if (!this.srvInfo[key]) await this.srvResolve(m.name);
          const info = this.srvInfo[key];
          if (!info || info.none) continue;
          const r = await this.srvExec(m.name, info.node, [info.script, info.main], 30000);
          this.srv[key] = { ...JSON.parse(r.stdout), name: m.name, label: m.label };
        } catch (e) {
          this.srv[key] = { ...(this.srv[key] ?? {}), name: m.name, label: m.label, error: String(e?.message ?? e) };
        }
      }
      this.srvLoading = false;
      cx.notify();
    });
  }

  tickServers(cx) {
    if (this.showId() || !["home", "servers"].includes(this.tab)) return;
    const gap = Object.keys(this.srvPending).length ? 3000 : 8000;
    if (Date.now() - this.srvAt >= gap) this.loadServers(cx);
  }

  // --- the services: read, probe, start, stop ------------------------------------------------------------------------------------
  async loadSvcConfig() {
    let list = null;
    try {
      const text = await Promise.resolve(readTextFile(SVC_FILE.replace(/^~/, home())));
      if (text) list = JSON.parse(text);
    } catch (_e) { list = null; }
    if (!Array.isArray(list)) { list = SVC_SEED; await this.writeSvcConfig(list); }   // (first time: the two dev servers of the person, now their own to change)
    this.svcList = list.filter((x) => x && x.id && x.name && Number(x.port) > 0);
    return this.svcList;
  }

  writeSvcConfig(list) {
    const file = SVC_FILE.replace(/^~/, home());
    return exec("sh", ["-c", 'mkdir -p "$(dirname "$1")" && printf %s "$2" > "$1"', "_", file, JSON.stringify(list, null, 2)], { timeout: 15000 }).catch(() => {});
  }

  async loadSvcState() {
    await this.loadSvcConfig();
    await Promise.all(this.svcList.map(async (x) => {
      try { this.svcState[x.id] = (await this.srvExec(x.machine ?? null, "bash", ["-c", SVC_PROBE, "_", String(x.port), x.id], 15000)).stdout.trim() || "down"; }
      catch (_e) { this.svcState[x.id] = "down"; }
    }));
  }

  // running | starting | stopping | stopped: the port says, a pending action makes it starting or stopping until the port agrees (2 min at most)
  svcStatus(x) {
    const up = this.svcState[x.id] === "up";
    if (up && this.svcFail[x.id]) delete this.svcFail[x.id];   // (it came up after all, or somebody started it by hand)
    const p = this.srvPending[`svc|${x.id}`];
    if (!p) return up ? "running" : this.svcFail[x.id] ? "failed" : "stopped";
    if ((p.want === "up" && up) || (p.want === "down" && !up) || Date.now() - p.at > 300000) { delete this.srvPending[`svc|${x.id}`]; return up ? "running" : "stopped"; }
    return p.want === "up" ? "starting" : "stopping";
  }

  svcColor(t, st) { return st === "running" ? (t.ansi2 ?? t.done) : st === "failed" ? t.blocked : st === "stopped" ? t.dim : t.working; }

  // A start that did not work: its process is gone and the port does not answer (after 6 s: the process needs a moment to show), or the port did not
  // answer within 2 minutes. The log says why; the card shows it in red, and a topic in review carries the short version, once per start.
  async checkSvcFailures() {
    for (const x of this.svcList) {
      const p = this.srvPending[`svc|${x.id}`];
      if (!p || p.want !== "up") continue;
      const tok = this.svcState[x.id], age = Date.now() - p.at;
      if (tok === "up" || !((tok === "down" && age > 6000) || age > 120000)) continue;
      delete this.srvPending[`svc|${x.id}`];
      const lines = await this.svcLogLines(x);
      const reason = tok === "down" ? "the start command ended and the server did not come up" : `the server did not answer on port ${x.port} within 2 minutes`;
      this.svcFail[x.id] = { at: Date.now(), reason, lines };
      this.svcFailTopic(x, reason, lines);
    }
  }

  // The last meaningful lines of a service's log (the shell's own noise left out).
  async svcLogLines(x) {
    try {
      const r = await this.srvExec(x.machine ?? null, "sh", ["-c", 'tail -n 80 "${HOME:?}/.cache/bhote/svc/$1.log" 2>/dev/null', "_", x.id], 15000);
      return r.stdout.split("\n").map((l) => l.replace(/\x1b\[[0-9;]*m/g, "").trim())
        .filter((l) => l && !/can't change option: zle|Ctrl-C stops all|^\(eval\)/.test(l)).slice(-6);
    } catch (_e) { return []; }
  }

  // The failure as a topic of the project (bhote add), the log's key line as its description (140 characters at most).
  svcFailTopic(x, reason, lines) {
    const key = `${x.id}|${this.svcFail[x.id]?.at}`;
    if (this.svcTopicDone?.has(key)) return;
    (this.svcTopicDone ??= new Set()).add(key);
    const hit = lines.findIndex((l) => /error|not found|failed|cannot|can't|refused|ENOENT|fatal|exception|denied/i.test(l));
    const parts = hit >= 0 ? lines.slice(hit, hit + 2) : lines.slice(-2);
    let desc = (parts.join(" – ") || reason).replace(/\s+/g, " ");
    if (desc.length > 138) desc = `${desc.slice(0, 137)}…`;
    const proj = this.projects.find((q) => norm(q.title) === norm(x.name))?.title;
    exec("bhote", ["add", `${x.name} did not start on ${this.machineLabel(x.machine ?? null)}`, "-d", desc, "-s", "review", ...(proj ? ["-p", proj] : [])], { timeout: 30000 }).catch(() => {});
  }

  svcDismiss(cx, x) { delete this.svcFail[x.id]; cx.notify(); }

  svcDo(cx, x, what) {
    const at = Date.now();
    const m = x.machine ?? null;
    const args = [x.id, String(x.port)];
    const stop = () => this.srvExec(m, "bash", ["-c", SVC_STOP, "_", ...args, x.stop ?? ""], 60000);
    const start = () => this.srvExec(m, "bash", ["-c", SVC_START, "_", ...args, x.start], 30000);
    delete this.svcFail[x.id];
    this.srvPending[`svc|${x.id}`] = { want: what === "stop" ? "down" : "up", at };
    cx.notify();
    cx.spawn(async (cx) => {
      try { if (what !== "start") await stop(); if (what !== "stop") await start(); } catch (_e) { /* the next probe shows what is there */ }
      this.srvAt = 0;
      this.loadServers(cx);
    });
  }

  svcOpen(x) { exec("open", [x.url || `http://localhost:${x.port}`]).catch(() => {}); }

  svcLog(x) { exec("open", ["-t", `${home()}/.cache/bhote/svc/${x.id}.log`]).catch(() => {}); }

  // The machines as the Servers tab and the tile show them: this computer first, then the others (by their alias), each with what the IMS look found and the
  // services that run on it (a service belongs to one machine: Bilendo on the Mac is not Bilendo on Omarchy).
  serverGroups() {
    const out = this.srvMachines().map((m) => ({ m, d: this.srv[this.srvKey(m.name)] ?? null, svcs: this.svcList.filter((x) => (x.machine ?? null) === (m.name ?? null)) }));
    for (const x of this.svcList) {   // (a machine that is not connected right now still has its services)
      if (!out.some((g) => (g.m.name ?? null) === (x.machine ?? null))) out.push({ m: { name: x.machine, label: this.machineLabel(x.machine) }, d: null, svcs: this.svcList.filter((y) => y.machine === x.machine) });
    }
    return out;
  }

  // A service as a card of the Servers tab: its state, buttons, and what it is started with.
  svcCard(t, x) {
    const st = this.svcStatus(x), color = this.svcColor(t, st);
    const busy = st === "starting" || st === "stopping", up = st === "running" || st === "stopping";
    return v_flex().id(`svcard-${x.id}`).gap(6).mx(10).mb(8).px(12).py(10).rounded(8).bg(t.surface).border(1).border_color(t.border)
      .child(h_flex().gap(8).items_center()
        .child(busy ? div().w(9).flex_none().text_color(color).text_size(10).frames(SPIN, 90) : div().w(8).h(8).rounded(4).flex_none().bg(color))
        .child(div().font_semibold().text_color(t.foreground).child(x.name))
        .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.muted).child(`:${x.port}`))
        .child(div().flex_none().text_size(11).text_color(color).child(st))
        .child(this.ibtn(t, `svcard-${x.id}-go`, up ? "restart" : "play", (cx) => this.svcDo(cx, x, up ? "restart" : "start"), { dim: busy, color: up ? t.muted : (t.ansi2 ?? t.done) }))
        .when(up, (el) => el.child(this.ibtn(t, `svcard-${x.id}-stop`, "stop", (cx) => this.svcDo(cx, x, "stop"), { dim: busy, hover: t.blocked })))
        .when(!x.machine, (el) => el.child(this.ibtn(t, `svcard-${x.id}-log`, "log", () => this.svcLog(x), { color: t.muted })))
        .when(st === "running" && !x.machine, (el) => el.child(this.ibtn(t, `svcard-${x.id}-launch`, "launch", () => this.svcOpen(x), { color: t.accent }))))
      .child(div().pl(16).text_size(11.5).text_color(t.dim).child(`start: ${x.start || "–"}${x.stop ? `  ·  stop: ${x.stop}` : ""}`))
      .when(st === "failed" && !!this.svcFail[x.id], (el) => el.child(v_flex().gap(3).ml(16).p(8).rounded(6).bg(tint(t.blocked, "1f", t.surface)).border(1).border_color(tint(t.blocked, "55", t.border))
        .child(h_flex().gap(8).items_center().child(div().flex_1().text_size(12).font_semibold().text_color(t.blocked).child(`Could not start: ${this.svcFail[x.id].reason}`))
          .child(div().id(`svcfail-x-${x.id}`).w(20).h(20).flex_none().flex().items_center().justify_center().rounded(5).text_size(11).text_color(t.dim).cursor_pointer()
            .hover((e) => e.bg(t.hover).text_color(t.foreground)).on_click((_e, cx) => this.svcDismiss(cx, x)).child("✕")))
        .children(this.svcFail[x.id].lines.slice(-3).map((l) => div().truncate().text_size(11.5).text_color(t.muted).child(l)))
        .child(div().text_size(11).text_color(t.dim).child("A topic in review has the short version."))));
  }

  // A service as a chip (the overview) or the same with more room (the Servers tab).
  svcChip(t, x) {
    const st = this.svcStatus(x);
    const color = this.svcColor(t, st);
    const busy = st === "starting" || st === "stopping";
    const up = st === "running" || st === "stopping";
    const id = `svc-${x.id}`;
    return h_flex().id(id).gap(0).items_center().pl(8).pr(3).py(3).rounded(8).bg(t.chrome).border(1).border_color(up && !busy ? tint(color, "55", t.border) : t.border)
      .child(busy ? div().w(9).flex_none().text_color(color).text_size(10).frames(SPIN, 90) : div().w(8).h(8).rounded(4).flex_none().bg(color))
      .child(div().id(`${id}-label`).pl(2).pr(6).text_size(12).font_semibold().text_color(up ? t.foreground : t.muted).child(x.name)
        .when(st === "running" && !x.machine, (el) => el.cursor_pointer().hover((e) => e.text_color(t.accent)).on_click(this.guard(() => this.svcOpen(x)))))
      .child(this.ibtn(t, `${id}-go`, up ? "restart" : "play", (cx) => this.svcDo(cx, x, up ? "restart" : "start"), { dim: busy, color: up ? t.muted : (t.ansi2 ?? t.done) }))
      .when(up, (el) => el.child(this.ibtn(t, `${id}-stop`, "stop", (cx) => this.svcDo(cx, x, "stop"), { dim: busy, hover: t.blocked })))
      .when(st === "running" && !x.machine, (el) => el.child(this.ibtn(t, `${id}-launch`, "launch", () => this.svcOpen(x), { color: t.accent })));
  }

  // The state of a system: running (server, client and portal online), partial, stopped; starting or stopping while an action is under way.
  slotState(key, sys) {
    const on = sys.apps.filter((a) => a.status === "online").length;
    const state = on === 0 ? "stopped" : on >= 3 ? "running" : "partial";
    const p = this.srvPending[`${key}|${sys.slot}`];
    if (!p) return state;
    if ((p.want === "up" && state === "running") || (p.want === "down" && state === "stopped") || Date.now() - p.at > 120000) { delete this.srvPending[`${key}|${sys.slot}`]; return state; }
    return p.want === "up" ? "starting" : "stopping";
  }

  // Is the tunnel being opened or closed? (until the poll shows it, 60 s at most)
  tunBusy(tun) {
    const p = this.srvPending.tunnel;
    if (!p) return false;
    if (!tun || (p.want === "up") === !!tun.open || Date.now() - p.at > 60000) { delete this.srvPending.tunnel; return false; }
    return true;
  }

  // The tunnel of this computer to a machine: { open, host } or null.
  tunnelTo(name) {
    const tun = this.srv.local?.tunnels ?? [];
    return name == null ? null : tun.find((x) => x.host.endsWith(`@${name}`) || x.host === name) ?? null;
  }

  srvDo(cx, name, what, slot) {
    const key = this.srvKey(name);
    const systems = this.srv[key]?.systems ?? [];
    const at = Date.now();
    let cmd;
    if (what === "start" || what === "restart") { cmd = `ims-${slot}-start -q`; this.srvPending[`${key}|${slot}`] = { want: "up", at }; }
    else if (what === "stop") { cmd = `ims-${slot}-stop`; this.srvPending[`${key}|${slot}`] = { want: "down", at }; }
    else if (what === "startAll") { cmd = "ims-start -q"; for (const x of systems) this.srvPending[`${key}|${x.slot}`] = { want: "up", at }; }
    else if (what === "stopAll") { cmd = "ims-stop"; for (const x of systems) this.srvPending[`${key}|${x.slot}`] = { want: "down", at }; }
    else if (what === "tunnelOpen") { cmd = "ims-tunnels-start"; this.srvPending.tunnel = { want: "up", at }; }
    else if (what === "tunnelClose") { cmd = "ims-tunnels-stop"; this.srvPending.tunnel = { want: "down", at }; }
    else return;
    cx.notify();
    cx.spawn(async (cx) => {
      try { await this.srvExec(name, "zsh", ["-ic", `_ims_node24 >/dev/null 2>&1; ${cmd}`], 180000); } catch (_e) { /* the next poll shows what is there */ }
      this.srvAt = 0;
      this.loadServers(cx);
    });
  }

  // What a system offers to open: its client, portal and API, those that run. Another machine's systems are reached through the tunnel.
  launchItems(name, sys) {
    const on = new Set(sys.apps.filter((a) => a.status === "online").map((a) => a.role));
    return [
      ...(on.has("client") ? [{ type: "url", url: `http://localhost:${sys.client}`, label: `Client  ·  localhost:${sys.client}` }] : []),
      ...(on.has("portal") ? [{ type: "url", url: `http://localhost:${sys.portal}`, label: `Portal  ·  localhost:${sys.portal}` }] : []),
      ...(on.has("server") ? [{ type: "url", url: `http://localhost:${sys.port}`, label: `API  ·  localhost:${sys.port}` }] : []),
    ];
  }

  canLaunch(name, sys) {
    if (!this.launchItems(name, sys).length) return false;
    return name == null || !!this.tunnelTo(name)?.open;
  }

  openLaunch(name, sys) {
    const items = this.launchItems(name, sys);
    if (items.length === 1) exec("open", [items[0].url]).catch(() => {});
    else this.openMenu(items, this.anchor);
  }

  // A small icon button; it remembers where it is (for a menu) and never counts as a click on what is behind it.
  ibtn(t, id, icon, onClick, o = {}) {
    return div().id(id).w(o.w ?? 20).h(22).flex_none().flex().items_center().justify_center().rounded(5).text_color(o.color ?? t.muted).opacity(o.hidden ? 0 : o.dim && !o.spin ? 0.3 : 1)
      .when(!o.dim && !o.hidden, (el) => el.cursor_pointer().hover((e) => e.bg(t.hover).text_color(o.hover ?? t.foreground))
        .on_mouse_down("left", (e, cx) => { this.anchor = this.anchorOf(e); cx.stop_propagation(); })
        .on_click(this.guard((_e, cx) => onClick(cx))))
      .child(o.spin ? div().text_size(12).text_color(t.working).frames(SPIN, 90) : svg(`mods/bhote/${icon}.svg`).size(o.size ?? 13));
  }

  // The colour of a system's state.
  slotColor(t, st) {
    return st === "running" ? (t.ansi2 ?? t.done) : st === "partial" ? t.ansi3 : st === "starting" || st === "stopping" ? t.working : t.dim;
  }

  // The controls of one system: its state, restart or start, stop, launch.
  slotChip(t, name, sys) {
    const key = this.srvKey(name);
    const st = this.slotState(key, sys);
    const color = this.slotColor(t, st);
    const busy = st === "starting" || st === "stopping";
    const up = st === "running" || st === "partial" || st === "stopping";
    const id = `srv-${key}-${sys.slot}`;
    const hovered = this.srvHover === id;
    return h_flex().id(id).gap(0).items_center().pl(8).pr(3).py(3).rounded(8).bg(t.chrome)
      .on_hover((h, cx) => { if (h) this.srvHover = id; else if (this.srvHover === id) this.srvHover = null; cx.notify(); }).border(1).border_color(up && !busy ? tint(color, "55", t.border) : t.border)
      .child(busy ? div().w(9).flex_none().text_color(color).text_size(10).frames(SPIN, 90) : div().w(8).h(8).rounded(4).flex_none().bg(color))
      .child(div().id(`${id}-label`).pl(2).pr(6).text_size(12).font_semibold().text_color(up ? t.foreground : t.muted).child(sys.slot)
        .when(this.canLaunch(name, sys), (el) => el.cursor_pointer().hover((e) => e.text_color(t.accent))
          .on_mouse_down("left", (e, cx) => { this.anchor = this.anchorOf(e); cx.stop_propagation(); })
          .on_click(this.guard(() => this.openLaunch(name, sys)))))
      .child(this.ibtn(t, `${id}-go`, up ? "restart" : "play", (cx) => this.srvDo(cx, name, up ? "restart" : "start", sys.slot), { dim: busy, color: up ? t.muted : (t.ansi2 ?? t.done) }))
      .when(up, (el) => el.child(this.ibtn(t, `${id}-stop`, "stop", (cx) => this.srvDo(cx, name, "stop", sys.slot), { dim: busy, hover: t.blocked })))
      .when(this.canLaunch(name, sys), (el) => el.child(this.ibtn(t, `${id}-launch`, "launch", () => this.openLaunch(name, sys), { color: t.accent })));
  }

  // RAM and CPU of a machine as two small bars with their figures.
  machineMeter(t, m) {
    const gb = (b) => (b / 2 ** 30).toFixed(1);
    const ram = m.ramTotal ? m.ramUsed / m.ramTotal : 0;
    const bar = (frac, color) => div().h(4).w(30).flex_none().rounded(2).bg(t.border).overflow_hidden().child(div().h(4).w(`${Math.round(Math.min(1, frac) * 100)}%`).bg(color));
    const hot = (f) => (f > 0.85 ? t.blocked : f > 0.65 ? t.ansi3 : (t.ansi2 ?? t.done));
    return h_flex().gap(8).items_center().flex_shrink(1).min_w(0).text_size(11).text_color(t.muted)
      .child(h_flex().gap(4).items_center().child(div().child("RAM")).child(bar(ram, hot(ram))).child(div().text_color(t.dim).child(`${gb(m.ramUsed)}/${gb(m.ramTotal)}G`)))
      .child(h_flex().gap(4).items_center().child(div().child("CPU")).child(bar(m.cpu / 100, hot(m.cpu / 100))).child(div().text_color(t.dim).child(`${Math.round(m.cpu)}%`)));
  }

  // The tile on top of the overview, across both columns: for each machine its load and the controls of its systems, and the tunnel.
  serverTile(t) {
    const machines = this.serverGroups().filter((g) => g.d?.systems?.length || g.svcs.length);   // (per machine: its IMS systems and its services)
    const waiting = this.srvMachines().some((m) => !this.srvInfo[this.srvKey(m.name)]);
    if (!machines.length && !waiting) return null;   // no IMS on any machine, no service
    const svcColor = (x) => this.svcColor(t, this.svcStatus(x));
    const small = this.compactTiles();
    const tun = (this.srv.local?.tunnels ?? [])[0];
    const tunPending = this.tunBusy((this.srv.local?.tunnels ?? [])[0]);
    let shut = false;
    try { shut = shared("bhote.srvShut") !== false; } catch (_e) { shut = true; }
    const toggle = (_e, cx) => { setShared("bhote.srvShut", !shut); cx.notify(); };
    if (shut) {
      // one line: a dot per system (green: runs, amber: partly, grey: stopped), per machine
      const dots = ({ m, d, svcs }) => h_flex().gap(4).items_center()
        .child(div().text_size(11.5).font_semibold().text_color(t.foreground).child(m.label))
        .children((d?.systems ?? []).map((sys) => div().w(8).h(8).rounded(4).flex_none().bg(this.slotColor(t, this.slotState(this.srvKey(m.name), sys)))))
        .children(svcs.length && d?.systems?.length ? [div().w(1).h(10).mx(2).flex_none().bg(t.border)] : [])   // (the services after the IMS systems of the machine)
        .children(svcs.map((x) => div().w(8).h(8).rounded(4).flex_none().bg(svcColor(x))));
      return h_flex().id("tile-servers").flex_none().gap(10).items_center().px(12).py(7).rounded(12).cursor_pointer().bg(t.surface).border(1).border_color(t.border)
        .hover((e) => e.bg(t.raised)).on_click(this.guard(toggle))
        .child(div().text_size(11).font_semibold().text_color(t.dim).child("SERVERS"))
        .children(machines.map((g) => dots(g)))
        .child(div().flex_1())
        .when(!!tun, (el) => el.child(tunPending ? div().text_size(12).text_color(t.working).frames(SPIN, 90) : svg(tun.open ? "mods/bhote/plug.svg" : "mods/bhote/unplug.svg").size(14).text_color(tun.open ? (t.ansi2 ?? t.done) : t.dim)))
        .child(svg("mods/bhote/chevron-down.svg").size(14).text_color(t.muted));
    }
    return v_flex().id("tile-servers").flex_none().gap(8).p(small ? 10 : 12).rounded(12).bg(t.surface).border(1).border_color(t.border)
      .child(h_flex().id("srv-head").gap(8).items_center().cursor_pointer().on_click(this.guard(toggle))
        .child(div().w(8).h(8).rounded(4).flex_none().bg(machines.some((x) => (x.d?.systems ?? []).some((y) => y.running) || x.svcs.some((y) => this.svcStatus(y) === "running")) ? (t.ansi2 ?? t.done) : t.dim))
        .child(div().flex_1().text_size(11).font_semibold().text_color(t.dim).child("SERVERS"))
        .child(svg("mods/bhote/chevron-up.svg").size(14).text_color(t.muted))
        .when(!!tun, (el) => el.child(this.ibtn(t, "srv-tunnel", tun.open ? "plug" : "unplug", (cx) => this.srvDo(cx, null, tun.open ? "tunnelClose" : "tunnelOpen"), { dim: tunPending, spin: tunPending, color: tun.open ? (t.ansi2 ?? t.done) : t.dim, hover: tun.open ? t.blocked : t.foreground }))))
      .children(machines.map(({ m, d, svcs }) => v_flex().gap(6)
        .child(h_flex().gap(8).items_center()
          .child(div().flex_none().font_semibold().text_size(12.5).text_color(t.foreground).child(m.label))
          .when(!!d?.machine, (el) => el.child(this.machineMeter(t, d.machine))))
        .child(h_flex().gap(6).flex_wrap().children([...(d?.systems ?? []).map((sys) => this.slotChip(t, m.name, sys)), ...svcs.map((x) => this.svcChip(t, x))]))))
      .when(!machines.length, (el) => el.child(div().text_size(12).text_color(t.dim).child("Looking for the servers...")));
  }

  // --- the runbooks tile: for each project the stages of the way a change goes, as circles ---------------------------------------------------------
  // Empty: not done (or not reached yet), green: done, orange: under way (a build, tests, an approval that waits), red: failed. Which change a project
  // shows: a deployment that runs, else a pull request that failed, one whose checks run, the newest open one, the newest merged one.
  runbookItems() {
    const out = [];
    for (const proj of this.projects) {
      if (!proj.ci) continue;
      const prs = this.prs.filter((x) => x.project === proj.title && x.kind === "pr");
      const open = prs.filter((x) => !x.merged).sort((a, b) => (b.updated || 0) - (a.updated || 0));
      const run = this.deploysAll().filter((x) => this.projectOf(x.project) === proj.title && ["running", "queued", "approval"].includes(x.state))[0];
      const failed = open.find((x) => x.checks.some((c) => c.state === "failed"));
      const live = open.find((x) => x.checks.some((c) => c.state === "running"));
      const merged = prs.filter((x) => x.merged).sort((a, b) => b.merged - a.merged)[0];
      const pr = failed ?? live ?? open[0] ?? merged ?? null;
      let stages, ref, rank;
      if (run) { stages = this.runStages(run, "d"); ref = this.whatOf(run).text || run.label; rank = 0; }
      else if (pr) { stages = this.stagesOf(pr); ref = `#${pr.number} ${pr.title}`; rank = failed ? 1 : live ? 2 : open[0] ? 3 : 5; }
      else { stages = runbookStages(proj, { hasPR: false, tests: "none", merged: false, target: "" }); ref = ""; rank = 9; }
      if (!stages) continue;
      const at = run ? run.time : pr ? (pr.merged || pr.updated || 0) : 0;   // (when the change last moved: the collapsed tile shows the latest)
      out.push({ project: proj.title, ref, stages: runbookPhases(stages), rank, at });
    }
    return out.sort((a, b) => a.rank - b.rank);
  }

  // One circle for a stage.
  stageCircle(t, state, size) {
    const orange = "#f08a24";
    const color = state === "ok" ? (t.ansi2 ?? t.done) : state === "failed" ? t.blocked : state === "running" || state === "approval" ? orange : null;
    if (!color) return div().w(size).h(size).flex_none().rounded(size / 2).border(1.5).border_color(t.dim);
    if (color !== orange) return div().w(size).h(size).flex_none().rounded(size / 2).bg(color);
    // under way: the orange circle pulses (a slow sine of the clock; the panel is drawn again every 100 ms while one is on screen)
    this.pulsing = true;
    const now = Date.now();
    const k = 0.5 + 0.5 * Math.sin((now / 1100) * 2 * Math.PI);
    // and a ring that goes out from it and fades, as the ping of a notification dot: it grows to 2.6 times the circle in 1.6 s
    const ph = (now % 1600) / 1600;
    const rs = size * (1 + 1.6 * ph), off = (size - rs) / 2;
    return div().relative().w(size).h(size).flex_none()
      .child(div().absolute().left(off).top(off).w(rs).h(rs).rounded(rs / 2).border(1.5).border_color(color).opacity(0.75 * (1 - ph)))
      .child(div().absolute().left(0).top(0).w(size).h(size).rounded(size / 2).bg(color).opacity(0.35 + 0.65 * k));
  }

  runbookTile(t) {
    const items = this.runbookItems();
    if (!items.length) return null;
    let shut = true;
    try { shut = shared("bhote.rbShut") !== false; } catch (_e) { shut = true; }
    const toggle = (_e, cx) => { setShared("bhote.rbShut", !shut); cx.notify(); };
    const busy = items.some((x) => x.stages.some((y) => y.state === "running" || y.state === "approval")), bad = items.some((x) => x.stages.some((y) => y.state === "failed"));
    const mark = bad ? t.blocked : busy ? "#f08a24" : (t.ansi2 ?? t.done);
    const chev = svg(shut ? "mods/bhote/chevron-down.svg" : "mods/bhote/chevron-up.svg").size(14).text_color(t.muted);
    if (shut) {
      // one line: only the latest run, its project, what it is and the circles of its stages
      const latest = [...items].sort((a, b) => b.at - a.at)[0];
      const ref = latest.ref.length > 34 ? `${latest.ref.slice(0, 33)}…` : latest.ref;
      return h_flex().id("tile-runbooks").flex_none().gap(12).items_center().px(12).py(8).rounded(12).cursor_pointer().bg(t.surface).border(1).border_color(t.border).overflow_hidden()
        .hover((e) => e.bg(t.raised)).on_click(this.guard(toggle))
        .child(div().flex_none().text_size(11).font_semibold().text_color(t.dim).child("RUNBOOKS"))
        .child(div().flex_none().text_size(11.5).font_semibold().text_color(t.foreground).child(latest.project))
        .child(h_flex().gap(3).items_center().flex_none().children(latest.stages.map((y) => this.stageCircle(t, y.state, 9))))
        .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.dim).child(ref))
        .child(chev);
    }
    return v_flex().id("tile-runbooks").flex_none().gap(10).p(12).rounded(12).bg(t.surface).border(1).border_color(t.border)
      .child(h_flex().id("rb-head").gap(8).items_center().cursor_pointer().on_click(this.guard(toggle))
        .child(div().w(8).h(8).rounded(4).flex_none().bg(mark))
        .child(div().flex_1().text_size(11).font_semibold().text_color(t.dim).child("RUNBOOKS"))
        .child(chev))
      .children(items.map((x, k) => v_flex().gap(8).pt(k ? 10 : 0).when(k > 0, (el) => el.border_t(1).border_color(t.border))
        .child(h_flex().gap(8).items_baseline().child(div().flex_none().font_semibold().text_size(12.5).text_color(t.foreground).child(x.project))
          .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.muted).child(x.ref)))
        // a stepper from edge to edge: the first circle at the left end, the last at the right end, the others evenly between (the columns at the ends are
        // narrower by half a step: their circle sits at the edge); the name is under its circle, the first one flush left, the last one flush right
        .child(h_flex().w_full().items_start().children(x.stages.map((y, i) => {
          const n = x.stages.length, first = i === 0, last = i === n - 1;
          const done = (z) => z && z.state === "ok";
          const line = (on) => div().flex_1().h(2).rounded(1).bg(on ? (t.ansi2 ?? t.done) : t.border);
          const base = y.name, sub = y.note;
          const active = y.state === "running" || y.state === "approval" || y.state === "failed";
          const weight = first || last ? 0.5 + (6 * Math.max(1, n - 1)) / 370 : 1;
          const place = (el) => (first ? el.justify_start() : last ? el.justify_end() : el.justify_center());
          return v_flex().flex_grow(weight).flex_basis(0).min_w(0).gap(4).when(first, (c) => c.items_start()).when(last, (c) => c.items_end()).when(!first && !last, (c) => c.items_center())
            .child(h_flex().w_full().items_center()
              .children(first ? [] : [line(done(x.stages[i - 1]))])
              .child(this.stageCircle(t, y.state, 12))
              .children(last ? [] : [line(done(y))]))
            .child(place(div().w_full().flex()).child(div().truncate().text_size(10).font_semibold().text_color(active || y.state === "ok" ? t.foreground : t.dim).child(base)))
            .when(!!sub, (e) => e.child(place(div().w_full().flex()).child(div().truncate().text_size(9).text_color(t.dim).child(sub))));
        })))));
  }

  // The status page (the Servers tab): the machines with their load, the systems with their apps, the tunnel.
  serversPage(t) {
    const rows = [];
    const fmtDur = (ms) => { const m = Math.floor(ms / 60000); return m < 60 ? `${m}m` : m < 1440 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${Math.floor(m / 1440)}d ${Math.floor((m % 1440) / 60)}h`; };
    const mb = (b) => (b >= 2 ** 30 ? `${(b / 2 ** 30).toFixed(1)} GB` : `${Math.round(b / 2 ** 20)} MB`);
    const machines = this.serverGroups().filter((g) => g.d || g.svcs.length);
    if (!machines.length) return [empty(t, "", "Looking for the servers", "Asking this computer and the other machines for their IMS systems and services", true)];
    const tun = (this.srv.local?.tunnels ?? [])[0];
    if (tun) {
      rows.push(div().flex_none().pt(8).child(this.dividerCaption(t, "Tunnel")));
      rows.push(h_flex().gap(10).items_center().mx(10).mb(6).px(12).py(9).rounded(8).bg(t.surface).border(1).border_color(t.border)
        .child(this.icon(t, tun.open ? "plug" : "unplug", tun.open ? (t.ansi2 ?? t.done) : t.dim, 16))
        .child(v_flex().flex_1().min_w(0).gap(1)
          .child(div().text_color(t.foreground).child(tun.open ? `Open to ${tun.host.split("@").pop()}` : "Closed"))
          .child(div().text_size(11).text_color(t.muted).child(`${tun.host} · port block ${tun.block}: the other machine's systems appear as localhost`)))
        .child(this.btn(t, "srv-tunnel-page", this.tunBusy(tun) ? (tun.open ? "Closing…" : "Opening…") : tun.open ? "Close" : "Open", (_e, cx) => this.tunBusy(tun) ? null : this.srvDo(cx, null, tun.open ? "tunnelClose" : "tunnelOpen"), { primary: !tun.open })));
    }
    for (const { m, d, svcs } of machines) {
      const key = this.srvKey(m.name);
      rows.push(div().flex_none().pt(8).child(this.dividerCaption(t, m.label)));
      if (!d) { for (const x of svcs) rows.push(this.svcCard(t, x)); continue; }   // (no IMS here, or it does not answer yet: the services still show)
      if (d.error && !d.machine) { rows.push(div().mx(10).mb(6).text_size(11).text_color(t.blocked).child(d.error.slice(0, 160))); for (const x of svcs) rows.push(this.svcCard(t, x)); continue; }
      rows.push(v_flex().gap(5).mx(10).mb(8).px(12).py(9).rounded(8).bg(t.surface).border(1).border_color(t.border)
        .child(this.machineMeter(t, d.machine))
        .child(div().text_size(11).text_color(t.dim).child(`${d.machine.platform === "darwin" ? "macOS" : d.machine.platform} · ${d.machine.cores} cores · load ${d.machine.load.toFixed(2)} · up ${fmtDur(d.machine.up)}`)));
      for (const sys of d.systems) {
        const st = this.slotState(key, sys);
        const color = this.slotColor(t, st);
        const up = st === "running" || st === "partial" || st === "stopping";
        rows.push(v_flex().gap(6).mx(10).mb(8).px(12).py(10).rounded(8).bg(t.surface).border(1).border_color(t.border)
          .child(h_flex().gap(8).items_center()
            .child(div().w(8).h(8).rounded(4).flex_none().bg(color))
            .child(div().font_semibold().text_color(t.foreground).child(sys.slot))
            .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.muted).child(`⎇ ${sys.branch}${sys.db ? ` · ${sys.db}` : ""}`))
            .child(div().flex_none().text_size(11).text_color(color).child(st === "running" ? `${mb(sys.rss)} · ${sys.cpu.toFixed(0)}%` : st))
            .child(this.ibtn(t, `srv-${key}-${sys.slot}-p-go`, up ? "restart" : "play", (cx) => this.srvDo(cx, m.name, up ? "restart" : "start", sys.slot), { color: up ? t.muted : (t.ansi2 ?? t.done) }))
            .when(up, (el) => el.child(this.ibtn(t, `srv-${key}-${sys.slot}-p-stop`, "stop", (cx) => this.srvDo(cx, m.name, "stop", sys.slot), { hover: t.blocked })))
            .when(this.canLaunch(m.name, sys), (el) => el.child(this.ibtn(t, `srv-${key}-${sys.slot}-p-launch`, "launch", () => this.openLaunch(m.name, sys), { color: t.accent }))))
          .children(sys.apps.map((a) => h_flex().gap(8).pl(16).items_center().text_size(11.5)
            .child(div().w(7).h(7).rounded(4).flex_none().bg(a.status === "online" ? (t.ansi2 ?? t.done) : a.status === "errored" ? t.blocked : t.dim))
            .child(div().w(52).flex_none().text_color(t.foreground).child(a.role))
            .child(div().w(44).flex_none().text_color(t.dim).child(a.role === "server" ? String(sys.port) : a.role === "client" ? String(sys.client) : a.role === "portal" ? String(sys.portal) : ""))
            .child(div().flex_1().min_w(0).text_color(t.muted).child(a.status === "online" ? `${a.cpu.toFixed(0)}% · ${mb(a.rss)} · up ${fmtDur(a.up)}${a.restarts ? ` · ↻${a.restarts}` : ""}` : a.status)))));
      }
      for (const x of svcs) rows.push(this.svcCard(t, x));   // the dev servers of this machine, beside its IMS systems
    }
    return rows;
  }

  // --- the overview: four large tiles, 2 x 2 -----------------------------------------------------------------------------------
  // A tile: its name, one big figure, a line under it, a short list. A click opens the tab behind it.
  // Small tiles (only the figure and what it says, no short lines): what the person chose with the button, else a panel that is not tall.
  compactTiles() {
    try { const v = shared("bhote.compact"); return typeof v === "boolean" ? v : !!this.compactHome; } catch (_e) { return !!this.compactHome; }
  }

  // The button that folds the tiles to their small form and back.
  openSettings() { openOverlay({ mod: "bhote", props: { settings: true }, kind: "dialog", modal: true, width: 1032, height: 768 }); }

  // The settings row at the foot of the panel, under Done: always there, on the overview and in every tab (like the rows under the agents).
  settingsDock(t) {
    // the look of the DONE caption above: same left edge (px 10), same size and colour; the gear is a drawn icon, left-aligned like the first letter
    const v = (this.version ?? "").replace(/^(\d+\.\d+\.\d+)\.\d+/, "$1");   // (0.9.3.1 Preview: the build number is not for the panel)
    return h_flex().id("settings-dock").flex_none().h(36).px(10).gap(8).items_center().cursor_pointer().border_t(1).border_color(t.border).bg(t.chrome)
      .text_color(t.dim).hover((e) => e.bg(t.hover).text_color(t.foreground))
      .on_click(this.guard(() => this.openSettings()))
      .child(svg("mods/bhote/gear.svg").size(14).flex_none())
      .child(div().flex_1().text_size(11).font_semibold().child("SETTINGS"))
      .child(h_flex().gap(6).items_center().flex_none()   // the branding
        .child(div().text_size(12).font_semibold().text_color(t.accent).child("bhote"))
        .when(!!v, (e) => e.child(div().text_size(11).text_color(t.dim).child(v))));
  }

  tilesToggle(t) {
    const small = this.compactTiles();
    return div().id("tiles-toggle").w(28).h(26).flex_none().flex().items_center().justify_center().rounded(6).cursor_pointer()
      .bg(t.raised).border(1).border_color(t.border).hover((e) => e.bg(t.hover))
      .on_click(this.guard(() => setShared("bhote.compact", !small)))
      .child(svg(small ? "mods/bhote/chevron-down.svg" : "mods/bhote/chevron-up.svg").size(14).text_color(t.muted));
  }

  tile(t, id, o) {
    const small = this.compactTiles();   // small: a smaller tile, only the figure and what it says; the three short lines are left out
    return v_flex().id(`tile-${id}`).flex_1().h(small ? 118 : 238).min_w(0).overflow_hidden().gap(small ? 2 : 4).p(small ? 10 : 12).rounded(12).cursor_pointer()
      .bg(t.surface).border(1).border_color(o.border ?? t.border).hover((e) => e.bg(t.raised))
      .on_click((_e, cx) => this.setTab(cx, id))
      .child(h_flex().flex_none().gap(6).items_center()
        .child(div().w(8).h(8).rounded(4).flex_none().bg(o.accent ?? t.dim))
        .child(div().flex_1().min_w(0).truncate().text_size(11).font_semibold().text_color(t.dim).child(o.title.toUpperCase()))
        .when(!!o.badge, (el) => el.child(div().px(6).rounded(8).text_size(10).bg("#a8162e").text_color("#ffffff").child(String(o.badge)))))
      .child(h_flex().flex_none().items_end().gap(8).pt(small ? 0 : 4)
        .child(div().text_size(small ? 24 : 32).font_semibold().text_color(o.bigColor ?? t.foreground).child(String(o.big)))
        // the small tile has no room for the time in the bar: it stands beside the figure
        .when(small && !!o.bar && o.bar.remain != null, (el) => el.child(div().pb(small ? 4 : 6).text_size(11.5).text_color(t.working).child(left(o.bar.remain)))))
      .child(div().flex_none().truncate().text_size(small ? 11.5 : 12).text_color(t.muted).child(o.label ?? ""))
      .when(!(small && o.bar), (el) => el.child(div().flex_none().truncate().text_size(small ? 10.5 : 11).text_color(t.dim).child(o.sub ?? "")))   // the small tile has room for the bar or the line, not both
      .when(!!o.bar, (el) => el.child(h_flex().flex_none().gap(8).items_center().pt(2)
        .child(div().flex_1().h(5).rounded(3).bg(t.border).overflow_hidden().child(div().h(5).w(`${Math.round(o.bar.frac * 100)}%`).rounded(3).bg(t.working)))
        .child(div().flex_none().text_size(10.5).text_color(t.working).child(`${Math.round(o.bar.frac * 100)}%${o.bar.remain != null && !small ? ` · ${left(o.bar.remain)}` : ""}`))))
      .when(!small, (el) => el.child(div().flex_none().h(1).my(6).bg(t.border))
        .child(v_flex().flex_1().gap(6).min_h(0).overflow_hidden().children((o.rows ?? []).slice(0, 3).map((r) => h_flex().gap(7).items_center().text_size(11.5)
          .child(div().w(6).h(6).rounded(3).flex_none().bg(r.color ?? t.dim))
          .child(div().flex_1().min_w(0).truncate().text_color(t.foreground).child(r.text))
          .when(!!r.right, (e) => e.child(div().flex_none().text_size(10.5).text_color(r.rightColor ?? t.dim).child(r.right))))))
        .when(!(o.rows ?? []).length, (e) => e.child(div().text_size(11.5).text_color(t.dim).child(o.empty ?? "nothing yet"))));
  }

  // The deployments that run, for the left panel (which does not ask CI itself): the branch they are for, what they are, how far they are.
  publishDeploys() {
    const rows = this.deploysAll().filter((x) => x.state === "running" || x.state === "queued").map((x) => {
      const e = this.runEstimate(x, "d");
      return { branch: x.branch ?? "", project: x.project, label: this.whatOf(x).text || x.label, frac: e.frac ?? null, remain: e.remain ?? null };
    }).filter((x) => x.branch);
    let old = null;
    try { old = shared("bhote.depRun"); } catch (_e) { old = null; }
    if (JSON.stringify(old ?? []) !== JSON.stringify(rows)) { try { setShared("bhote.depRun", rows); } catch (_e) { /* not shared */ } }
  }

  // The deployment that runs for a branch (an agent's), or null.
  deployOf(branch) {
    if (!branch) return null;
    try { return (shared("bhote.depRun") ?? []).find((x) => x.branch === branch) ?? null; } catch (_e) { return null; }
  }

  // A ring that fills, the percentage in it.
  ring(t, frac, color, size) {
    const pct = Math.max(0, Math.min(100, Math.round(frac * 100)));
    return div().relative().w(size).h(size).flex_none()
      .child(svg(`mods/bhote/ring-${Math.round(pct / 5) * 5}.svg`).size(size).text_color(color))
      .child(div().absolute().inset_0().flex().items_center().justify_center().text_size(size >= 34 ? 10.5 : 8.5).font_semibold().text_color(color).child(`${pct}%`));
  }

  // The progress of the runs that go now (a test run, a deployment): the mean of those with a figure and the longest time left; null when none.
  runsBar(rows, kind) {
    const est = rows.filter((x) => x.state === "running" || x.state === "queued").map((x) => this.runEstimate(x, kind));
    const fr = est.filter((e) => e.frac != null).map((e) => e.frac);
    if (!fr.length) return null;
    const rem = est.filter((e) => e.remain != null).map((e) => e.remain);
    return { frac: fr.reduce((a, b) => a + b, 0) / fr.length, remain: rem.length ? Math.max(...rem) : null };
  }

  // The checks of the open pull requests that still run: the mean share of finished checks, the longest time left; null when none runs.
  prsBar(prs) {
    const est = prs.filter((p) => p.checks.some((c) => c.state === "running")).map((p) => this.checkEstimate(p.checks)).filter((e) => e.frac != null);
    if (!est.length) return null;
    const rem = est.filter((e) => e.remain != null).map((e) => e.remain);
    return { frac: est.reduce((a, e) => a + e.frac, 0) / est.length, remain: rem.length ? Math.max(...rem) : null };
  }

  homeView(t) {
    const count = (g) => this.topics.filter((x) => x.group === g && !x.dismissed).length;
    const n = { review: count("review"), now: count("now"), testing: count("testing"), deploying: count("deploying"), finished: count("finished"), next: count("next"), waiting: count("waiting"), later: count("later") };
    const open = Object.values(n).reduce((a, b) => a + b, 0);
    const topicRows = [["review", "Review", t.ansi4], ["now", "At work", t.working], ["testing", "Testing", t.ansi6], ["deploying", "Waiting for deployment", t.ansi3],
      ["finished", "Finished", t.done], ["next", "Next", t.ansi3], ["waiting", "Waiting", t.ansi3], ["later", "Later", t.dim]]
      .filter(([k]) => n[k] > 0).map(([k, label, color]) => ({ text: label, color, right: String(n[k]), rightColor: t.muted }));

    const prs = this.prs.filter((p) => !p.merged);
    const failing = (p) => p.checks.filter((c) => c.state === "failed").length;
    const running = (p) => p.checks.filter((c) => c.state === "running").length;
    const prsFailed = prs.filter((p) => failing(p) > 0).length;
    const prRows = [...prs].sort((a, b) => failing(b) - failing(a)).map((p) => ({ text: `${p.number ? `#${p.number} ` : ""}${p.title}`,
      color: failing(p) ? t.blocked : running(p) ? t.working : t.done, right: failing(p) ? `${failing(p)} failed` : running(p) ? "running" : "", rightColor: failing(p) ? t.blocked : t.working }));

    const newest = new Map();
    for (const x of this.deploysAll()) { const k = `${x.project}|${x.label}`; if (!newest.has(k)) newest.set(k, x); }
    const deploys = [...newest.values()];
    const dcolor = (x) => (x.state === "ok" ? t.done : x.state === "failed" ? t.blocked : x.state === "running" ? t.working : x.state === "approval" ? t.ansi3 : t.dim);
    const dword = (x) => (x.state === "ok" ? "deployed" : x.state);
    const depRunning = this.deploysAll().filter((x) => x.state === "running" || x.state === "queued" || x.state === "approval").length;
    const depFailed = deploys.filter((x) => x.state === "failed").length;
    // the context of the tile: which projects are watched, which are switched off, and when the last one was
    const watched = this.projects.filter((q) => q.ci && !this.deployOff.has(q.title)).map((q) => q.title);
    const switchedOff = this.projects.filter((q) => q.ci && this.deployOff.has(q.title)).length;
    const lastOne = deploys.filter((x) => x.state !== "running" && x.state !== "waiting")[0];

    const { shown } = this.testsShown();
    const tFailed = shown.filter((x) => x.state === "failed"), tPassed = shown.filter((x) => x.state === "ok");
    const tRunning = shown.filter((x) => x.state === "running" || x.state === "queued").length;
    const tRows = [...tFailed, ...tRunning ? shown.filter((x) => x.state === "running") : [], ...tPassed].map((x) => ({ text: `${x.project} · ${x.label}`,
      color: x.state === "failed" ? t.blocked : x.state === "running" ? t.working : t.done, right: ago(x.time), rightColor: t.dim }));

    // four tiles of one size stay at the top; under them the topics scroll
    const srvTile = this.serverTile(t);
    const tiles = v_flex().flex_none().p(10).gap(10).border_b(1).border_color(t.border)
      .children(srvTile ? [srvTile] : [])
      .children(this.runbookTile(t) ? [this.runbookTile(t)] : [])
      .child(h_flex().flex_none().gap(10)
        .child(this.tile(t, "topics", { title: "Topics", accent: n.review ? t.ansi4 : t.ansi3, big: open, label: open === 1 ? "open topic" : "open topics",
          sub: `${n.review + n.now + n.testing + n.deploying + n.finished} active · ${n.next} next · ${n.waiting} waiting`, rows: topicRows, empty: "no topics yet" }))
        .child(this.tile(t, "prs", { title: "Pull requests", accent: prsFailed ? t.blocked : t.done, badge: prsFailed || null, big: prs.length, bigColor: prsFailed ? t.blocked : t.foreground,
          label: prs.length === 1 ? "open pull request" : "open pull requests", bar: this.prsBar(prs),
          sub: !this.prAt ? "asking GitHub..." : prsFailed ? `${prsFailed} with failing checks` : prs.length ? "all checks fine" : "nothing open",
          rows: prRows, empty: this.prAt ? "no open pull requests" : "loading", border: prsFailed ? "#a8162e" : undefined })))
      .child(h_flex().flex_none().gap(10)
        .child(this.tile(t, "deploys", { title: "Deployments", accent: depFailed ? t.blocked : depRunning ? t.working : t.done, badge: depFailed || null,
          big: depRunning ? depRunning : deploys.length ? dword(deploys[0]) : "–", bigColor: depRunning ? t.working : deploys.length ? dcolor(deploys[0]) : t.dim,
          label: depRunning ? `${(() => { const r = this.deploysAll().find((x) => x.state === "running" || x.state === "queued"); return r ? this.whatOf(r).text || r.label : ""; })()}` : lastOne ? `last: ${this.whatOf(lastOne).text || lastOne.label}` : "nothing running",
          bar: this.runsBar(this.deploysAll(), "d"),
          sub: [watched.length ? `${watched.join(", ")} watched` : "no project watched", switchedOff ? `${switchedOff} off` : ""].filter(Boolean).join(" · "),
          rows: deploys.map((x) => ({ text: x.state === "ok" || x.state === "failed" ? `${x.project} · ${this.whatOf(x).text || x.label}` : `${x.project} · ${x.label}`,
            color: dcolor(x), right: x.state === "running" || x.state === "waiting" ? dword(x) : ago(x.time), rightColor: dcolor(x) })),
          empty: watched.length ? "no pipeline seen on the deploy branches yet" : "all projects are switched off (DEPLOY_PROJECTS_OFF)", border: depFailed ? "#a8162e" : undefined }))
        .child(this.tile(t, "tests", { title: "Tests", accent: tFailed.length ? t.blocked : tRunning ? t.working : t.done, badge: tFailed.length || null,
          big: tFailed.length || tRunning || tPassed.length, bigColor: tFailed.length ? t.blocked : tRunning ? t.working : t.done, label: tFailed.length ? "failed" : tRunning ? "running" : "passed",
          bar: this.runsBar(shown, "t"), sub: `${tPassed.length} passed · ${tRunning} running`, rows: tRows, empty: "no test runs seen", border: tFailed.length ? "#a8162e" : undefined })));
    const list = this.right();
    const bar = this.batchBar(t, "topics", this.topics.filter((x) => x.group !== "done" && !x.dismissed).map((x) => x.id), this.tilesToggle(t));
    return v_flex().id("home-root").size_full().bg(t.chrome)
      .on_size_change((size, cx) => { const small = size.height < 1000; if (small !== !!this.compactHome) { this.compactHome = small; cx.notify(); } })
      .child(tiles).children(bar ? [bar] : []).children(this.adding ? [this.addRow(t)] : [])
      .child(v_flex().flex_1().min_h(0).overflow_y_scrollbar().children(list))
      .children([...this.docks(t), this.settingsDock(t)]);
  }

  setTab(cx, id) {
    this.tab = id;
    setShared("bhote.tab", id);
    cx.notify();
  }

  // A run of a monitor as a card: its state, project and workflow, the branch and title, the bar while it runs, the time.
  // A run that passed: one slim green row (the small round badge ticks it, the suite, what passed, when, the ... menu). The runs that
  // matter, those that run or failed, keep the big card.
  testRow(t, id, x, archived) {
    const key = this.runKey(x);
    const picked = this.sel().tests.has(key);
    const canceled = x.state === "canceled";
    const fixed = x.state === "fixed";
    const green = canceled ? t.dim : fixed ? t.ansi3 : (t.ansi2 ?? t.done);
    const what = canceled ? "canceled" : fixed ? "failed · fixed" : x.title || "passed";
    const items = () => {
      const k = this.runKey(x);
      return [
        ...(x.url ? [{ type: "url", url: x.url, label: "Open in CI" }] : []),
        ...(archived ? [{ type: "hide", mode: "restore", key: k, label: "Restore" }] : [{ type: "hide", mode: "archive", key: k, label: "Archive" }]),
        { type: "hide", mode: "discard", key: k, label: "Discard", danger: true },
      ];
    };
    return h_flex().id(id).gap(8).items_center().bg(t.surface).border(1).border_color(picked ? t.accent : t.border).rounded(8).mx(8).mb(4).pl(7).pr(4).py(4)
      .hover((e) => e.bg(t.raised)).on_click(this.guard(() => this.open(x.url)))
      .child(div().id(`ck-${id}`).w(22).h(22).flex_none().flex().items_center().justify_center().rounded(11).cursor_pointer()
        .bg(picked ? t.accent : tint(green, "26", t.raised)).border(1).border_color(picked ? t.accent : tint(green, "55", t.border))
        .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
        .on_click(this.guard((_e, cx) => { this.toggleSel("tests", key); cx.notify(); }))
        .child(picked ? svg("mods/bhote/check.svg").size(11).text_color(t.accent_foreground ?? "#ffffff") : canceled ? this.stateIcon(t, "canceled", green, 12) : this.icon(t, "ok", green, 12)))
      .child(div().flex_1().min_w(0).truncate().text_size(12).text_color(t.muted).child(`${x.label}${x.project ? `  ·  ${x.project}` : ""}${archived ? "  ·  archived" : ""}`))
      .child(div().flex_none().text_size(11.5).font_semibold().text_color(canceled ? t.muted : green).child(what))
      .child(div().flex_none().text_size(10.5).text_color(t.dim).child(clock(x.time)))
      .child(this.optionsBtn(t, `op-${id}`, items));
  }

  // A test run as a result card, not a to-do: a round badge in front with the state (it is also how a run is ticked for the batch actions),
  // the suite and what it belongs to, the figure that counts on the right (how many passed, how many failed, the percentage while it runs),
  // and under it a bar that shows the result: green for what passed, red for what failed; while it runs, its progress.
  testCard(t, id, x, archived) {
    if (x.state === "ok" || x.state === "canceled" || x.state === "fixed") return this.testRow(t, id, x, archived);
    const key = this.runKey(x);
    const picked = this.sel().tests.has(key);
    const color = x.state === "ok" ? t.done : x.state === "failed" ? t.blocked : x.state === "running" ? t.working : x.state === "approval" ? t.ansi3 : t.dim;
    const word = { ok: "passed", failed: "failed", running: "running", approval: "approval", canceled: "canceled", queued: "queued" }[x.state] ?? x.state;
    const count = (re) => { const m = re.exec(x.title ?? ""); return m ? Number(m[1].replace(/[.,]/g, "")) : null; };
    const passed = count(/(\d[\d.,]*)\s*passed/i), failedN = count(/(\d[\d.,]*)\s*failed/i);
    const sum = (passed ?? 0) + (failedN ?? 0);
    const live = x.state === "running" || x.state === "queued";
    const est = live ? this.runEstimate(x, "t") : null;
    let big, label;
    if (live) { big = est.frac != null ? `${Math.round(est.frac * 100)}%` : "…"; label = est.remain != null ? left(est.remain) : "running"; }
    else if (x.state === "failed") { big = failedN != null ? String(failedN) : "failed"; label = failedN != null ? "failed" : ""; }
    else if (passed != null) { big = String(passed); label = "passed"; }
    else { big = word; label = ""; }
    const okPct = live ? 0 : sum > 0 ? (passed ?? 0) / sum * 100 : x.state === "ok" ? 100 : 0;
    const badPct = live ? 0 : sum > 0 ? (failedN ?? 0) / sum * 100 : x.state === "failed" ? 100 : 0;
    const items = () => {
      const k = this.runKey(x);
      return [
        ...(x.url ? [{ type: "url", url: x.url, label: "Open in CI" }] : []),
        ...(archived ? [{ type: "hide", mode: "restore", key: k, label: "Restore" }] : [{ type: "hide", mode: "archive", key: k, label: "Archive" }]),
        { type: "hide", mode: "discard", key: k, label: "Discard", danger: true },
      ];
    };
    const meta = [x.project, x.branch ? `⎇ ${x.branch}` : "", clock(x.time), archived ? "archived" : ""].filter(Boolean).join(" · ");
    return h_flex().id(id).gap(10).items_center().bg(t.surface).border(1).border_color(picked ? t.accent : x.state === "failed" ? "#a8162e" : t.border).rounded(10).mx(8).mb(6).pl(10).pr(6).py(10)
      .hover((e) => e.bg(t.raised)).on_click(this.guard(() => this.open(x.url)))
      // the badge: the state; a tick when the run is selected
      .child(div().id(`ck-${id}`).w(22).h(22).flex_none().flex().items_center().justify_center().rounded(11).cursor_pointer()
        .bg(picked ? t.accent : tint(color, "26", t.raised)).border(1).border_color(picked ? t.accent : tint(color, "55", t.border))
        .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
        .on_click(this.guard((_e, cx) => { this.toggleSel("tests", key); cx.notify(); }))
        .child(picked ? svg("mods/bhote/check.svg").size(11).text_color(t.accent_foreground ?? "#ffffff") : this.stateIcon(t, x.state, color, 12)))
      .child(v_flex().flex_1().min_w(0).gap(4)
        .child(div().truncate().text_size(12).font_semibold().text_color(t.foreground).child(x.label))
        .child(div().truncate().text_size(11).text_color(t.muted).child(meta))
        .child(div().h(4).w_full().rounded(2).bg(t.border).overflow_hidden().flex()
          .child(div().h(4).w(`${live ? Math.round((est.frac ?? 0) * 100) : okPct}%`).bg(live ? t.working : (t.ansi2 ?? t.done)))
          .when(badPct > 0, (e) => e.child(div().h(4).w(`${badPct}%`).bg(t.blocked))))
        .when(!!this.runStages(x, "t"), (el) => el.child(this.pathRow(t, this.runStages(x, "t")))))
      .child(v_flex().flex_none().items_end().gap(0)
        .child(div().text_size(11.5).font_semibold().text_color(color).child(big))
        .when(!!label, (el) => el.child(div().text_size(10.5).text_color(t.dim).child(label))))
      .child(this.optionsBtn(t, `op-${id}`, items));
  }

  // A run of a test or a build as a card, laid out like a pull request: the title row (the checkbox of a test or the state icon of a
  // build, the title, the ... menu), under it what it belongs to and when, then the result: the state icon with the outcome, or the
  // progress bar and the time left while it runs.
  // A deployment that is over and fine (or canceled) as one slim line, like a passed test run.
  deployRow(t, id, x) {
    const canceled = x.state === "canceled";
    const green = canceled ? t.dim : (t.ansi2 ?? t.done);
    const w = this.whatOf(x);
    const what = [x.label, w.text || (x.branch ? `⎇ ${x.branch}` : "")].filter(Boolean).join("  ·  ");
    return h_flex().id(id).gap(8).items_center().bg(t.surface).border(1).border_color(t.border).rounded(8).mx(8).mb(4).pl(7).pr(4).py(4)
      .hover((e) => e.bg(t.raised)).on_click(this.guard(() => this.open(x.url)))
      .child(div().w(22).h(22).flex_none().flex().items_center().justify_center().rounded(11)
        .bg(tint(green, "26", t.raised)).border(1).border_color(tint(green, "55", t.border))
        .child(canceled ? this.stateIcon(t, "canceled", green, 12) : this.icon(t, "ok", green, 12)))
      .child(div().flex_1().min_w(0).truncate().text_size(12).text_color(t.muted).child(what))
      .child(div().flex_none().text_size(11.5).font_semibold().text_color(canceled ? t.muted : green).child(canceled ? "canceled" : "deployed"))
      .child(div().flex_none().text_size(10.5).text_color(t.dim).child(clock(x.time)))
      .when(!!x.url, (el) => el.child(this.optionsBtn(t, `op-${id}`, () => [{ type: "url", url: x.url, label: "Open in CI" }])));
  }

  monitorCard(t, id, x, kind, archived) {
    if (kind === "t") return this.testCard(t, id, x, archived);
    if (x.state === "ok" || x.state === "canceled") return this.deployRow(t, id, x);
    const key = this.runKey(x);
    const isTest = kind === "t";
    const picked = isTest && this.sel().tests.has(key);
    const color = x.state === "ok" ? t.done : x.state === "failed" ? t.blocked : x.state === "running" ? t.working : x.state === "approval" || x.state === "waiting" ? t.ansi3 : t.dim;
    const word = { ok: isTest ? "passed" : "deployed", failed: "failed", running: "running", approval: "waiting for approval", waiting: "waiting for the build", canceled: "canceled", queued: "queued" }[x.state] ?? x.state;
    // a test: the project (the name of a worktree) and the branch; a build: the branch and who started it
    const meta = isTest ? [x.project, x.branch ? `⎇ ${x.branch}` : ""].filter(Boolean).join(" · ")
      : (() => { const w = this.whatOf(x); return [x.branch ? `⎇ ${x.branch}` : "", w.text, w.author].filter(Boolean).join(" · "); })();
    const outcome = x.state === "ok" ? (x.title && isTest ? x.title : word) : x.title && isTest ? `${word} · ${x.title}` : word;
    const indent = isTest ? 31 : 0;
    const items = () => {
      const k = this.runKey(x);
      return [
        ...(x.url ? [{ type: "url", url: x.url, label: "Open in CI" }] : []),
        ...(isTest && archived ? [{ type: "hide", mode: "restore", key: k, label: "Restore" }] : []),
        ...(isTest && !archived ? [{ type: "hide", mode: "archive", key: k, label: "Archive" }] : []),
        ...(isTest ? [{ type: "hide", mode: "discard", key: k, label: "Discard", danger: true }] : []),
      ];
    };
    return h_flex().id(id).bg(t.surface).border(1).border_color(picked ? t.accent : x.state === "failed" ? "#a8162e" : t.border).rounded(8).mx(8).mb(6)
      .hover((s) => s.bg(t.raised)).on_click(this.guard(() => this.open(x.url)))
      .child(div().w(3).flex_none().rounded(2).my(6).ml(6).bg(color))
      .child(v_flex().flex_1().min_w(0).gap(3).pl(8).pr(6).py(8)
        .child(h_flex().gap(7).items_center()
          .child(isTest ? this.check(t, `ck-${id}`, "tests", key, picked) : this.stateIcon(t, x.state, color, 15))
          .child(div().flex_1().min_w(0).truncate().font_semibold().text_color(t.foreground).child(x.label))
          .when(items().length > 0, (el) => el.child(this.optionsBtn(t, `op-${id}`, items))))
        .child(h_flex().gap(8).pl(indent).pr(4).items_center().text_size(11)
          .child(div().flex_1().min_w(0).truncate().text_color(t.muted).child(meta || " "))
          .child(div().flex_none().text_color(t.dim).child(clock(x.time))))
        .when(!isTest && !!this.runStages(x, kind), (el) => el.child(this.pathRow(t, this.runStages(x, kind))))
        .when(x.state === "running", (el) => {
          const e = this.runEstimate(x, kind);
          return el.child(h_flex().gap(8).pl(indent).items_center().text_size(11.5).text_color(t.working)
            .when(e.frac != null, (r) => r.child(progress(t, e.frac, t.working)).child(div().child(`${Math.round(e.frac * 100)}%`)))
            .when(e.remain != null, (r) => r.child(div().text_color(t.muted).child(left(e.remain))))
            .when(e.frac == null && e.remain == null, (r) => r.child(div().text_color(t.muted).child("running"))));
        })
        .when(x.state !== "running", (el) => el.child(h_flex().gap(6).pl(indent).items_center().text_size(12).text_color(color)
          .child(this.stateIcon(t, x.state, color, 14))
          .child(div().min_w(0).truncate().child(`${archived ? "archived · " : ""}${outcome}`)))));
  }

  // Right: next, waiting and parked topics as lines, the last done ones, then the deployments and tests.
  right() {
    const t = theme();
    const rows = [];
    if (this.error) rows.push(div().p(10).text_color(t.blocked).child(`bhote: ${this.error}`));
    // the topics by status, in the order of the work; no agent is shown (a topic meets one when it is started on it)
    const of = (g) => this.topics.filter((x) => x.group === g && !x.dismissed);
    const sections = [["review", "Review"], ["now", "Now"], ["testing", "Testing"], ["deploying", "Waiting for deployment"], ["finished", "Finished"],
      ["next", "Next"], ["waiting", "Waiting"], ["later", "Later"]].map(([k, label]) => [k, label, of(k)]);
    for (const [, label, list] of sections) {
      if (!list.length) continue;
      rows.push(this.head(t, `${label} · ${list.length}`));
      for (const x of list) rows.push(this.line(t, x, null, ""));
    }
    if (!rows.length) rows.push(empty(t, "☰", "No topics yet", 'Add one with  bhote add "Fix the login redirect"'));
    return rows;
  }

  render(_cx) {
    if (this.setItems !== undefined) return this.settingsView();
    if (this.hdPick !== undefined) return this.headlessView();
    if (this.menu) return this.menuView();
    if (this.side === "right" && this.showId()) return this.showView();
    if (this.side === "right" && this.tab === "home") return this.homeView(theme());
    const t = theme();
    if (this.side === "left") return v_flex().id("left-root").size_full().bg(t.chrome).text_size(13)
      .on_size_change((size) => { this.leftH = size.height; })
      .child(v_flex().flex_1().min_h(0).overflow_y_scrollbar().children(this.left()))
      .children(this.commitBar(t))
      .children(this.jobsBar(t))
      .child(this.newSpaceBar(t));
    let body;
    if (this.tab === "servers") body = this.serversPage(t);
    else if (this.tab === "prs") body = this.prsBody(t);
    else if (this.tab === "deploys") body = this.deploysAll().length ? this.monitorBody(t, this.deploysAll(), "d", "dp") : [empty(t, "⇪", "No deployments or builds seen", "Builds on open pull requests show up here. For deployments: if CircleCI rejects its token (401), set a new one:  bhote config set CIRCLECI_TOKEN <token>")];
    else if (this.tab === "tests") {
      const { shown, archived } = this.testsShown();
      body = shown.length ? this.monitorBody(t, shown, "t", "ts") : (archived.length ? [] : [empty(t, "◎", "No test runs seen", "CI runs and  bhote test run -- <command>  show up here")]);
      // ticked runs: the batch bar on top (its menu archives, restores or discards them)
      const bar = this.batchBar(t, "tests", [...shown, ...archived].map((x) => this.runKey(x)));
      if (bar) body.unshift(bar);
    }
    else {
      body = this.right();
      const bar = this.batchBar(t, "topics", this.topics.filter((x) => x.group !== "done" && !x.dismissed).map((x) => x.id));
      if (this.adding) body.unshift(this.addRow(t));
      if (bar) body.unshift(bar);
    }
    return v_flex().size_full().bg(t.chrome).text_size(13)
      .child(this.tabBar(t))
      .child(v_flex().flex_1().min_h(0).overflow_y_scrollbar().children(body))
      .children([...(this.tab === "tests" ? this.archiveDock(t) : this.tab === "topics" ? this.docks(t) : []), this.settingsDock(t)]);
  }
}
