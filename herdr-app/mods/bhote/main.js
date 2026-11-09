// bhote for herdr-app: the topics and agents panels as a mod (bhote 0.9.2.1 split layout).
// props().side "left":  the workspaces, then the cards (review, now, testing, deploying, finished; an agent without
//                       a card has one of its own: idle, working, needs you).
// props().side "right": the lines of the topics (next, waiting, later), the last done ones, the deployments and tests.
// Data: agents() / spaces() of herdr-app (live), the topic files and the monitor lists bhoted writes (files are the
// interface). Actions: the bhote CLI through exec(), herdr-app commands through run().

import { View, div, svg } from "gpui-kit";
import { v_flex, h_flex, InputState, Input, TextareaState, Textarea } from "gpui-base";
import { agents, spaces, run, theme, props, exec, home, watchFolder, shared, setShared, openOverlay, closeOverlay, machines, settings } from "herdr";

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
function parseMonitor(text) {
  return text.split("\n").filter(Boolean).map((l) => {
    const f = l.split("\x1f");
    const [branch, title] = (f[6] ?? "").split("|~|");
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
    out.push({ id: v.id, title: v.title || v.id, repos: (v.repos ?? "").split(",").map((x) => x.trim()).filter(Boolean) });
  }
  return out;
}

// A GitHub check of a pull request (a check run of Actions, or a commit status such as CircleCI's) as ok | failed | running | skipped.
function checkOf(c) {
  const name = c.name || c.context || c.workflowName || "check";
  const url = c.detailsUrl || c.targetUrl || "";
  const started = secOf(c.startedAt), ended = secOf(c.completedAt);
  if (c.__typename === "StatusContext") {
    const st = (c.state ?? "").toUpperCase();
    return { name, url, started, ended, state: st === "SUCCESS" ? "ok" : st === "FAILURE" || st === "ERROR" ? "failed" : "running" };
  }
  if ((c.status ?? "").toUpperCase() !== "COMPLETED") return { name, url, started, ended: 0, state: "running" };
  const cc = (c.conclusion ?? "").toUpperCase();
  return { name, url, started, ended, state: cc === "SUCCESS" ? "ok" : ["SKIPPED", "NEUTRAL"].includes(cc) ? "skipped" : "failed" };
}

// The runs of a branch without a pull request (GitHub Actions) as checks.
function runOf(r) {
  const state = r.status !== "completed" ? "running" : r.conclusion === "success" ? "ok" : ["skipped", "neutral"].includes(r.conclusion) ? "skipped" : "failed";
  return { name: r.workflowName || r.name || "run", url: r.url || "", started: secOf(r.createdAt), ended: 0, state };
}

const PR_FIELDS = "number,title,headRefName,baseRefName,url,isDraft,state,mergedAt,reviewDecision,statusCheckRollup";

const secOf = (iso) => { const n = Date.parse(iso ?? ""); return Number.isFinite(n) ? Math.floor(n / 1000) : 0; };

// This machine's name in bhote's topics: "Mac" on macOS (HOST), else the short hostname.
// A checkbox. Off: an empty rounded square with a clear border. On: filled with the accent, with a drawn check mark.
function checkbox(t, on) {
  return div().w(18).h(18).flex_none().rounded(5).flex().items_center().justify_center()
    .border(2).border_color(on ? t.accent : t.muted).bg(on ? t.accent : t.surface)
    .when(on, (el) => el.child(svg("mods/bhote/check.svg").size(12).text_color(t.accent_foreground ?? "#ffffff")));
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

export default class Bhote extends View {
  init(_props, cx) {
    if (props()?.menu) { this.menu = props().menu; return; }   // the Options popup: a list of items, nothing to load
    this.side = props()?.side === "left" ? "left" : "right";
    try { this.tab = localStorage.getItem("tab") ?? "topics"; } catch (_e) { this.tab = "topics"; }
    if (!["topics", "prs", "deploys", "tests"].includes(this.tab)) this.tab = "topics";
    this.topics = [];
    this.tests = [];
    this.deploys = [];
    this.error = null;
    this.loading = false;
    this.projects = [];
    this.prs = [];
    this.prLoading = false;
    this.prAt = 0;
    // the show view: which topic it showed last, edit mode with its fields, the delete confirmation
    this.shownId = null;
    this.showArchived = false;
    this.selecting = false;       // multi-select mode of the Tests tab
    this.selected = new Set();    // the run keys that are ticked
    this.editing = false;
    this.confirmDelete = false;
    if (this.side === "right") {
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
          's="${BHOTE_SHARED:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/bhote-$(id -u)}"; ' +
          'printf "\\037T\\n"; cat "$s/tests.list" 2>/dev/null; printf "\\037D\\n"; cat "$s/deploys.list" 2>/dev/null';
        const r = await exec("sh", ["-c", script], { timeout: 15000 });
        const [topics, rest = ""] = r.stdout.split("\x1fT\n");
        const [tests = "", deploys = ""] = rest.split("\x1fD\n");
        this.topics = parseTopics(topics);
        this.tests = parseMonitor(tests);
        this.deploys = parseMonitor(deploys);
        this.projects = parseProjects(topics);
        this.observeRows(this.tests, "t");
        this.observeRows(this.deploys, "d");
        if (this.side === "right" && !this.prAt) this.loadPRs(cx);
        this.error = null;
      } catch (e) {
        this.error = String(e?.message ?? e);
      }
      this.loading = false;
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
    const elapsed = Date.now() / 1000 - x.time;
    const med = this.typical(`${kind}|${x.project}|${x.label}`);
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

  cli(cx, args) {
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

    // 3. who works on it
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
    const deploys = project ? this.deploys.filter((r) => r.project === project.title).slice(0, 3) : [];
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

  // --- test runs a person archived or discarded: kept in the shared state, so they stay away after a restart -------------------
  runKey(x) { return `${x.project}|${x.label}|${x.time}`; }

  hidden() {
    try { const h = shared("bhote.hidden"); return { archived: new Set(h?.archived ?? []), discarded: new Set(h?.discarded ?? []) }; } catch (_e) { return { archived: new Set(), discarded: new Set() }; }
  }

  // The test runs the Tests tab shows (not archived, not discarded) and the archived ones.
  testsShown() {
    const h = this.hidden();
    return { shown: this.tests.filter((x) => !h.archived.has(this.runKey(x)) && !h.discarded.has(this.runKey(x))),
      archived: this.tests.filter((x) => h.archived.has(this.runKey(x)) && !h.discarded.has(this.runKey(x))) };
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

  topicItems(x) {
    return [{ type: "show", id: x.id, label: "Show topic" }, ...this.statusItems(x)];
  }

  // The button: it never counts as a click on the card behind it.
  optionsBtn(t, id, getItems) {
    return div().id(id).flex_none().px(8).py(2).rounded(5).text_size(11).cursor_pointer().text_color(t.muted).border(1).border_color(t.border)
      .hover((e) => e.bg(t.hover).text_color(t.foreground))
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .on_click(this.guard(() => this.openMenu(getItems()))).child("Options ▾");
  }

  // The foot of a card: its status caption at the bottom left, the Options button at the bottom right.
  footer(t, id, caption, color, getItems) {
    return h_flex().gap(8).pt(3).items_center()
      .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(color ?? t.muted).child(caption ?? ""))
      .when(!!getItems, (el) => el.child(this.optionsBtn(t, id, getItems)));
  }

  // A click that closes the Options popup (a click beside it) must not also be taken by the card under it: the popup instance
  // notes when it ends, and for a moment after that the cards ignore clicks.
  menuRecent() {
    try { return Date.now() - (shared("bhote.menuClosed") ?? 0) < 400; } catch (_e) { return false; }
  }

  guard(fn) {
    return (e, cx) => { if (!this.menuRecent()) return fn(e, cx); };
  }

  async close() {
    if (this.menu) setShared("bhote.menuClosed", Date.now());
  }

  // `above`: where the bar that opened it is ({ x, top }): the popup goes up from there, not down.
  openMenu(items, above) {
    if (!items.length) return;
    const rows = items.reduce((n, it) => n + (it.type === "head" ? 0.8 : 1), 0);
    const height = Math.round(rows * 30 + 14);
    const at = above ? { x: above.x, y: Math.max(8, Math.round(above.top - height + 11)) } : "pointer";   // (4 px over the bar, measured against the real frame)
    openOverlay({ mod: "bhote", props: { menu: items }, kind: "popup", width: 224, height, at });
  }

  menuView() {
    const t = theme();
    return v_flex().w_full().p(4).bg(t.popover).text_size(12.5).children(this.menu.map((it, i) => it.type === "head"
      ? div().px(10).pt(6).pb(2).text_size(11).text_color(t.dim).child(it.label.toUpperCase())
      : div().id(`mi-${i}`).px(10).py(6).rounded(5).cursor_pointer().text_color(it.danger ? t.blocked : t.foreground)
        .hover((e) => e.bg(t.hover)).on_click(() => this.runItem(it)).child(it.label)));
  }

  // What an item does; the popup closes after it (a started exec keeps running).
  runItem(it) {
    if (it.type === "url") exec("open", [it.url]).catch(() => {});
    else if (it.type === "status") exec("bhote", [it.status, it.id], { timeout: 30000 }).catch(() => {});
    else if (it.type === "create") exec("bhote", ["add", it.title, "-d", it.desc, "-s", "next"], { timeout: 30000 }).catch(() => {});
    else if (it.type === "focus") run("agent.focus", { terminal: it.terminal });
    else if (it.type === "newspace") run("space.new", { machine: it.machine });
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
  agentIcon(t, agent) {
    if (!agent) return div().w(14).flex_none().text_color(t.dim).child("◌");
    if (agent.status === "working") return div().w(14).flex_none().text_color(t.working).frames(SPIN, 90);
    if (agent.status === "blocked") return div().w(14).flex_none().text_color(t.blocked).child("◉");
    if (agent.status === "done") return div().w(14).flex_none().text_color(t.done).child("✓");
    return div().w(14).flex_none().text_color(t.dim).child("○");
  }

  // The small bars of the tests that run for the topic's branch (CI test jobs, two at most).
  testBars(topic) {
    if (!topic.branch) return [];
    return this.tests.filter((x) => x.state === "running" && x.branch === topic.branch && x.total > 0).slice(0, 2);
  }

  // One card: a bar in the status colour, the agent (icon, machine, name, age and state), its task, the live text of
  // a working agent and the branch with the small test bars on the right.
  card(t, id, o) {
    const { topic, agent, machine, st } = o;
    const dim = o.dim;
    const fg = dim ? t.dim : t.foreground;
    const sub = dim ? t.dim : t.muted;
    const bars = topic ? this.testBars(topic) : [];
    const age = topic?.updated ? ago(topic.updated) : "";
    const task = topic?.title || (agent?.title ?? "");
    const live = agent?.status === "working" && topic ? (agent.title ?? "") : "";
    // Every card has a ground: the active agent's is prominent (the selection colour, an accent border), every other one a
    // muted card surface with a thin border; an idle agent's text is dimmed, its ground stays.
    const active = !!o.selected;
    // a workspace without an agent (o.flat) has neither a ground nor a border: the panel shows through (the border keeps its width so the cards line up)
    const ground = active ? tint(t.accent, "40", t.selected) : o.flat ? t.chrome : t.surface;
    return h_flex().id(id).bg(ground).border(1).border_color(active ? t.accent : o.flat ? t.chrome : t.border)
      .hover((x) => x.bg(active ? tint(t.accent, "52", t.selected) : o.flat ? t.surface : t.raised)).mx(8).mb(6).rounded(8)
      .on_click(this.guard(() => { if (o.onClick) o.onClick(); else if (agent) run("agent.focus", { terminal: agent.address }); }))
      .child(div().w(3).flex_none().rounded(2).my(6).ml(6).bg(st.bar))
      .child(v_flex().flex_1().min_w(0).gap(3).px(10).py(8)
        .child(h_flex().gap(6).items_center()
          .child(this.agentIcon(t, agent))
          .when(!!machine, (el) => el.child(div().flex_none().text_size(11).text_color(t.ansi4).child(machine)))
          .child(div().flex_1().truncate().font_semibold().text_color(fg).child(o.name))
          .child(div().flex_none().text_size(11).text_color(sub).child(age ? `${age} · ` : ""))
          .child(div().flex_none().text_size(11).text_color(dim ? t.dim : st.color).child(st.word))
          .when(!!topic, (el) => el.child(div().id(`o-${id}`).flex_none().px(4).text_color(t.dim).hover((x) => x.text_color(t.foreground))
            .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
            .on_click(this.guard(() => this.openPage(topic.id))).child("›"))))
        .when(!!task, (el) => el.child(h_flex().gap(6).items_center()
          .child(div().flex_1().truncate().text_size(12).text_color(sub).child(`∟ ${task}`))
          .when(bars.length > 0, (e) => e.child(div().flex_none().text_size(11).text_color(t.working)
            .child(bars.map((x) => `${x.label.slice(0, 8)} ${bar(x.done, x.total)}`).join("  "))))))
        .when(!!live && live !== task, (el) => el.child(div().truncate().pl(14).text_size(11).text_color(t.working).child(live)))
        .when(!!(topic?.branch || o.branch), (el) => el.child(div().truncate().pl(14).text_size(11).text_color(sub).child(`⎇ ${topic?.branch || o.branch}`)))
        .when(!!o.selected && !!topic?.desc, (el) => el.child(div().pl(14).text_size(11).text_color(sub).child(topic.desc))));
  }

  // A new space is a new shell on a machine (its default shell, in its home). The bar at the bottom asks which machine first (with
  // one machine there is nothing to ask); the plus next to a machine's name makes one there at once.
  chooseMachine() {
    const ms = machines().filter((m) => m.layout !== "unavailable");
    if (ms.length <= 1) { run("space.new", { machine: ms[0]?.name ?? "" }); return; }
    // The bar sits at the bottom of the agent pane, under the brand row (46 px at zoom 1): its top is that row plus the pane's
    // height minus its own. Both heights are measured by the pane itself; the layout's px sizes follow the zoom.
    const zoom = Number(settings()?.zoom) || 1;
    const top = 46 * zoom + (this.leftH ?? 0) - (this.barH ?? 38);
    this.openMenu([{ type: "head", label: "New space on" }, ...ms.map((m) => ({ type: "newspace", machine: m.name ?? "", label: m.name ?? "This computer" }))],
      this.leftH ? { x: 10 * zoom, top } : null);
  }

  // The bar at the bottom of the agent pane: it stays there while the list scrolls.
  newSpaceBar(t) {
    return h_flex().id("new-space").flex_none().px(12).py(9).gap(8).items_center().cursor_pointer().border_t(1).border_color(t.border)
      .on_size_change((size) => { this.barH = size.height; })
      .text_size(12).text_color(t.muted).hover((e) => e.bg(t.hover).text_color(t.foreground))
      .on_click(() => this.chooseMachine())
      .child(div().child("＋")).child(div().flex_1().child("New space"));
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
    const cards = this.topics.filter((x) => CARD_ORDER.includes(x.group) && !x.dismissed);
    const byMachine = new Map();
    for (const s of sp) {
      const m = s.machine ?? "This computer";
      if (!byMachine.has(m)) byMachine.set(m, []);
      byMachine.get(m).push(s);
    }
    for (const [machine, spaceList] of byMachine) {
      const remote = spaceList[0]?.machine ?? null;
      rows.push(this.head(t, machine, div().id(`ns-${machine}`).px(7).py(1).rounded(4).text_size(14).cursor_pointer().text_color(t.dim)
        .hover((e) => e.bg(t.hover).text_color(t.foreground))
        .on_click(() => run("space.new", { machine: remote ?? "" })).child("＋")));
      for (const s of spaceList) {
        const mine = list.filter((a) => a.space === s.id);
        if (!mine.length) {
          // a workspace without an agent: a card too, quieter, a click shows the workspace
          rows.push(this.card(t, `s-${s.id}`, { topic: null, agent: null, machine: s.machine, name: s.name, branch: s.branch,
            st: { word: "", color: t.dim, bar: t.idle }, selected: s.active, dim: true, flat: true, onClick: () => run("space.select", { id: s.id }) }));
          continue;
        }
        for (const a of mine) {
          const topics = cards.filter((x) => this.agentOf(x, [a], machineOf)).sort((p, q) => CARD_ORDER.indexOf(p.group) - CARD_ORDER.indexOf(q.group));
          const machineName = machineOf(a);
          for (const x of topics) {
            rows.push(this.card(t, `c-${x.id}`, { topic: x, agent: a, machine: machineName, name: s.name, branch: s.branch, st: this.cardState(t, x, a), selected: a.focused }));
          }
          if (!topics.length) {
            const idle = a.status === "idle";
            const st = a.status === "working" ? { word: "working", color: t.working, bar: t.working }
              : a.status === "blocked" ? { word: "needs you", color: t.blocked, bar: t.blocked }
              : a.status === "done" ? { word: "finished", color: t.done, bar: t.done } : { word: "idle", color: t.dim, bar: t.idle };
            rows.push(this.card(t, `a-${a.address}`, { topic: null, agent: a, machine: machineName, name: s.name, branch: s.branch, st, selected: a.focused, dim: idle }));
          }
        }
      }
    }
    return rows;
  }

  line(t, x, cx, sub) {
    // A topic is a card like the agents' cards: a muted ground, a thin border, rounded.
    return h_flex().id(`t-${x.id}`).gap(8).items_start().bg(t.surface).border(1).border_color(t.border).rounded(8)
      .mx(8).mb(6).px(10).py(8).hover((s) => s.bg(t.raised))
      .on_click(this.guard(() => this.openPage(x.id)))
      .child(v_flex().flex_1().min_w(0).gap(2)
        .child(div().text_color(t.foreground).child(x.title))
        .when(!!sub, (el) => el.child(div().text_size(11).text_color(t.muted).child(sub)))
        .child(this.footer(t, `op-t-${x.id}`, this.cardState(t, x, null).word, this.cardState(t, x, null).color, () => this.topicItems(x))));
  }

  // Open and recently merged pull requests of the projects' repositories (gh), and the runs of branches that have no pull
  // request yet. Merged ones get the deployments that started after the merge (bhoted's deploys.list, by project and time).
  loadPRs(cx) {
    if (this.prLoading) return;
    this.prLoading = true;
    cx.spawn(async (cx) => {
      const out = [];
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
              out.push({ kind: "pr", project: p.title, slug, number: q.number, title: q.title, branch: q.headRefName, base: q.baseRefName, url: q.url, draft: !!q.isDraft, merged, review: q.reviewDecision || "", checks });
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
              out.push({ kind: "branch", project: p.title, slug, number: 0, title: b, branch: b, base: "", url: `https://github.com/${slug}/tree/${b}`, draft: false, merged: 0, review: "", checks });
            }
          }
        }
        this.prs = out;
        this.prAt = Date.now();
      } catch (e) {
        this.error = String(e?.message ?? e);
      }
      this.prLoading = false;
      cx.notify();
    });
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
  prCard(t, id, pr, o = {}) {
    const by = (st) => pr.checks.filter((c) => c.state === st);
    const failed = by("failed"), running = by("running"), ok = by("ok");
    const color = pr.merged ? t.ansi5 : failed.length ? t.blocked : running.length ? t.working : ok.length ? t.done : t.dim;
    const chip = pr.merged ? ["merged", t.ansi5] : pr.draft ? ["draft", t.dim]
      : pr.review === "APPROVED" ? ["approved", t.done] : pr.review === "CHANGES_REQUESTED" ? ["changes requested", t.blocked]
      : pr.review === "REVIEW_REQUIRED" ? ["review needed", t.muted] : pr.kind === "branch" ? ["no PR yet", t.dim] : null;
    const deploys = pr.merged ? this.deploysAfter(pr) : [];
    const mark = (x) => (x.state === "ok" ? "✓" : x.state === "failed" ? "✗" : x.state === "running" ? "◔" : x.state === "approval" ? "◐" : "○");
    const dcolor = (x) => (x.state === "ok" ? t.done : x.state === "failed" ? t.blocked : x.state === "approval" ? t.ansi3 : x.state === "running" ? t.working : t.dim);
    return h_flex().id(id).bg(t.surface).border(1).border_color(failed.length && !pr.merged ? "#a8162e" : t.border).rounded(8).mx(8).mb(6)
      .hover((x) => x.bg(t.raised)).on_click(this.guard(() => this.open(pr.url)))
      .child(div().w(3).flex_none().rounded(2).my(6).ml(6).bg(color))
      .child(v_flex().flex_1().min_w(0).gap(3).px(10).py(8)
        .child(h_flex().gap(6).items_center()
          .when(pr.number > 0, (el) => el.child(div().flex_none().text_color(t.dim).child(`#${pr.number}`)))
          .child(div().flex_1().min_w(0).truncate().font_semibold().text_color(t.foreground).child(pr.title)))
        .child(div().truncate().text_size(11).text_color(t.muted).child(`⎇ ${pr.branch} · ${pr.project}`))
        .when(pr.checks.length > 0 && running.length > 0, (el) => {
          const e = this.checkEstimate(pr.checks);
          return el.child(h_flex().gap(8).items_center().text_size(11).text_color(t.working)
            .child(progress(t, e.frac ?? 0, failed.length ? t.blocked : t.working))
            .child(div().child(`${e.finished}/${e.total} checks`))
            .when(e.remain != null, (r) => r.child(div().text_color(t.muted).child(left(e.remain)))));
        })
        .when(pr.checks.length > 0, (el) => el.child(h_flex().gap(10).text_size(12)
          .child(div().text_color(t.done).child(`✓ ${ok.length}`))
          .when(failed.length > 0, (e) => e.child(div().text_color(t.blocked).child(`✗ ${failed.length}`)))
          .when(running.length > 0, (e) => e.child(div().text_color(t.working).child(`◔ ${running.length}`)))))
        .when(pr.checks.length === 0, (el) => el.child(div().text_size(11).text_color(t.dim).child("no checks seen")))
        .children(failed.slice(0, 3).map((c) => div().truncate().pl(4).text_size(11).text_color(t.blocked).child(`✗ ${c.name}`)))
        .children(running.slice(0, 2).map((c) => div().truncate().pl(4).text_size(11).text_color(t.working).child(`◔ ${c.name}`)))
        .when(pr.merged > 0, (el) => el.child(v_flex().gap(2).pt(2)
          .child(div().text_size(11).text_color(t.dim).child(deploys.length ? `deployments after the merge · ${ago(pr.merged)}` : `merged ${ago(pr.merged)} · no deployment seen`))
          .children(deploys.map((x) => h_flex().gap(6).text_size(12)
            .child(div().w(12).flex_none().text_color(dcolor(x)).child(mark(x)))
            .child(div().flex_1().truncate().text_color(t.foreground).child(x.label))
            .child(div().flex_none().text_size(11).text_color(dcolor(x)).child(x.state === "ok" ? "deployed" : x.state))))))
        .child(this.footer(t, `op-${id}`, chip ? chip[0] : "", chip ? chip[1] : t.muted, () => {
          const topic = this.topicOf(pr);
          const failedNames = failed.map((c) => c.name);
          return [
            { type: "url", url: pr.url, label: pr.kind === "branch" ? "Open branch in GitHub" : "Open in GitHub" },
            ...(topic ? this.topicItems(topic) : []),
            ...(!topic && pr.number > 0 && failedNames.length ? [{ type: "create", title: `PR #${pr.number}: fix the failing checks`, desc: `${failedNames.join(", ")} fail on ${pr.branch}`.slice(0, 140), label: "Create topic for the failing checks" }] : []),
          ];
        })));
  }

  prsBody(t) {
    const rows = [];
    const open = this.prs.filter((x) => x.kind === "pr" && !x.merged);
    const branches = this.prs.filter((x) => x.kind === "branch");
    const merged = this.prs.filter((x) => x.merged).sort((a, b) => b.merged - a.merged);
    const sections = [["Open pull requests", open], ["Branches without a pull request", branches], ["Merged, last 14 days", merged]];
    for (const [label, list] of sections) {
      if (!list.length) continue;
      rows.push(this.head(t, `${label} · ${list.length}`));
      list.slice(0, 10).forEach((x, i) => rows.push(this.prCard(t, `pr-${label.length}-${i}`, x)));   // the ten newest
    }
    if (!rows.length) rows.push(this.prLoading || !this.prAt ? empty(t, "", "Loading pull requests", "Asking GitHub for your open pull requests", true) : empty(t, "⑂", "No open pull requests", "Your pull requests and the branches you work on show up here with their checks"));
    return rows;
  }

  // The tab bar of the right panel: Topics, Deployments, Tests; a monitor's tab shows what runs (orange) and what failed (red).
  tabBar(t, cx) {
    const prList = this.prs.filter((x) => !x.merged).map((x) => ({ state: x.checks.some((c) => c.state === "failed") ? "failed" : x.checks.some((c) => c.state === "running") ? "running" : "ok" }));
    const tabs = [["topics", "Topics", null], ["prs", "PRs", prList], ["deploys", "Deployments", this.deploys], ["tests", "Tests", this.testsShown().shown]];
    return h_flex().flex_none().gap(4).px(8).pt(8).pb(6).items_center().border_b(1).border_color(t.border)
      .children(tabs.map(([id, label, list]) => {
        const on = this.tab === id;
        const running = list ? list.filter((x) => x.state === "running" || x.state === "queued" || x.state === "approval").length : 0;
        const failed = list ? list.filter((x) => x.state === "failed").length : 0;
        return h_flex().id(`tab-${id}`).gap(5).px(9).py(4).rounded(6).items_center().text_size(12)
          .bg(on ? t.selected : t.chrome).text_color(on ? t.foreground : t.muted).hover((x) => x.bg(on ? t.selected : t.hover))
          .on_click((_e, cx) => this.setTab(cx, id))
          .child(label)
          .when(running > 0, (el) => el.child(div().text_size(11).text_color(t.working).child(String(running))))
          .when(failed > 0, (el) => el.child(div().px(4).rounded(8).text_size(10).bg("#a8162e").text_color("#ffffff").child(String(failed))));
      }));
  }

  // The runs of a monitor grouped by project: the project with the newest run first; each group says what runs and what failed
  // and shows its ten newest runs.
  monitorBody(t, rows, kind, prefix) {
    const titles = new Map(this.projects.map((p) => [p.title.toLowerCase(), p.title]));
    // local runs are named after their worktree ("IMS-wt1", "IMS (wt2)"): they belong to the project of that name
    const projectOf = (name) => { const base = name.replace(/[\s-]*\(?wt\d+\)?$/i, "").trim() || name; return titles.get(base.toLowerCase()) ?? base; };
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
      const list = all.filter((x) => x.state !== "ok" || ++passed <= 2);
      const hidden = all.length - list.length;
      const running = list.filter((x) => x.state === "running" || x.state === "queued" || x.state === "approval").length;
      const failed = list.filter((x) => x.state === "failed").length;
      out.push(this.head(t, `${project} · ${list.length}`, h_flex().gap(6).items_center()
        .when(hidden > 0, (el) => el.child(div().text_color(t.dim).child(`+${hidden} passed`)))
        .when(running > 0, (el) => el.child(div().text_color(t.working).child(`${running} running`)))
        .when(failed > 0, (el) => el.child(div().px(5).rounded(8).bg("#a8162e").text_color("#ffffff").child(`✗ ${failed}`))), true));
      list.slice(0, 10).forEach((x, i) => out.push(this.monitorCard(t, `${prefix}-${project}-${i}`, x, kind, false, kind === "t" ? { on: this.selecting, set: this.selected } : null)));
    }
    return out;
  }

  setTab(cx, id) {
    this.tab = id;
    try { localStorage.setItem("tab", id); } catch (_e) { /* a private window: the tab is not remembered */ }
    cx.notify();
  }

  // A run of a monitor as a card: its state, project and workflow, the branch and title, the bar while it runs, the time.
  monitorCard(t, id, x, kind, archived, sel) {
    const key = this.runKey(x);
    const picked = !!sel?.on && sel.set.has(key);
    const color = x.state === "ok" ? t.done : x.state === "failed" ? t.blocked : x.state === "running" ? t.working : x.state === "approval" ? t.ansi3 : t.dim;
    const mark = x.state === "ok" ? "✓" : x.state === "failed" ? "✗" : x.state === "approval" ? "◐" : "○";
    const word = x.state === "ok" ? "passed" : x.state === "failed" ? "failed" : x.state === "canceled" ? "canceled" : x.state;
    const detail = [x.branch ? `⎇ ${x.branch}` : "", x.title].filter(Boolean).join(" · ");
    return h_flex().id(id).bg(t.surface).border(1).border_color(picked ? t.accent : x.state === "failed" ? "#a8162e" : t.border).rounded(8).mx(8).mb(6)
      .hover((s) => s.bg(t.raised)).on_click(this.guard((_e, cx) => {
        if (sel?.on) { if (this.selected.has(key)) this.selected.delete(key); else this.selected.add(key); cx.notify(); } else this.open(x.url);
      }))
      .child(div().w(3).flex_none().rounded(2).my(6).ml(6).bg(color))
      .child(v_flex().flex_1().min_w(0).gap(3).px(10).py(8)
        .child(h_flex().gap(6).items_center()
          // in select mode the checkbox takes the place of the state icon
          .child(sel?.on ? checkbox(t, picked)
            : x.state === "running" ? div().w(14).flex_none().text_color(color).frames(SPIN, 90) : div().w(14).flex_none().text_color(color).child(mark))
          .child(div().flex_1().min_w(0).truncate().font_semibold().text_color(t.foreground).child(`${x.project} · ${x.label}`))
          .child(div().flex_none().text_size(11).text_color(t.dim).child(clock(x.time))))
        .when(!!detail, (el) => el.child(div().truncate().pl(18).text_size(11).text_color(t.muted).child(detail)))
        .when(x.state === "running", (el) => {
          const e = this.runEstimate(x, kind);
          return el.child(h_flex().gap(8).pl(18).items_center().text_size(11).text_color(t.working)
            .when(e.frac != null, (r) => r.child(progress(t, e.frac, t.working)).child(div().child(`${Math.round(e.frac * 100)}%`)))
            .when(e.remain != null, (r) => r.child(div().text_color(t.muted).child(left(e.remain))))
            .when(e.frac == null && e.remain == null, (r) => r.child(div().text_color(t.muted).child("running"))));
        })
        .child(this.footer(t, `op-${id}`, archived ? `${word} · archived` : word, color, sel?.on ? null : () => {
          const key = this.runKey(x);
          return [
            ...(x.url ? [{ type: "url", url: x.url, label: "Open in CI" }] : []),
            ...(kind === "t" && archived ? [{ type: "hide", mode: "restore", key, label: "Restore" }] : []),
            ...(kind === "t" && !archived ? [{ type: "hide", mode: "archive", key, label: "Archive" }] : []),
            ...(kind === "t" ? [{ type: "hide", mode: "discard", key, label: "Discard", danger: true }] : []),
          ];
        })));
  }

  // Right: next, waiting and parked topics as lines, the last done ones, then the deployments and tests.
  right() {
    const t = theme();
    const rows = [];
    if (this.error) rows.push(div().p(10).text_color(t.blocked).child(`bhote: ${this.error}`));
    // review, now, testing, deploying and finished topics whose agent is not here (another machine offline, a gone agent): the agent
    // pane only shows cards with an agent, so they are listed here
    const sp = spaces();
    const machineOf = (a) => sp.find((q) => q.id === a.space)?.machine ?? null;
    const live = agents();
    const orphan = this.topics.filter((x) => CARD_ORDER.includes(x.group) && !x.dismissed && !this.agentOf(x, live, machineOf));
    const sections = [
      ["active", "Without agent", orphan],
      ["next", "Next", this.topics.filter((x) => x.group === "next")],
      ["waiting", "Waiting", this.topics.filter((x) => x.group === "waiting")],
      ["later", "Later", this.topics.filter((x) => x.group === "later" && !x.dismissed)],
    ];
    for (const [key, label, list] of sections) {
      if (!list.length) continue;
      rows.push(this.head(t, `${label} · ${list.length}`));
      for (const x of list) rows.push(this.line(t, x, null, x.agent ?? ""));
    }
    const done = this.topics.filter((x) => x.group === "done").slice(0, 5);
    if (done.length) {
      rows.push(this.head(t, "Done"));
      for (const x of done) {
        // a done topic is a card like the others, quieter: a muted ground, a thin border, dimmed text; a click opens its page
        rows.push(v_flex().id(`dn-${x.id}`).gap(3).bg(t.surface).border(1).border_color(t.border).rounded(8).mx(8).mb(6).px(10).py(8)
          .hover((e) => e.bg(t.raised)).on_click(this.guard(() => this.openPage(x.id)))
          .child(h_flex().gap(8).items_center()
            .child(div().flex_none().text_size(12).text_color(t.done).child("✓"))
            .child(div().flex_1().min_w(0).truncate().text_size(12).text_color(t.muted).child(x.title)))
          .child(this.footer(t, `op-dn-${x.id}`, `done${x.agent ? ` · ${x.agent}` : ""}`, t.dim, () => this.topicItems(x))));
      }
    }
    if (!rows.length) rows.push(empty(t, "☰", "No topics yet", 'Add one with  bhote add "Fix the login redirect"'));
    return rows;
  }

  render(_cx) {
    if (this.menu) return this.menuView();
    if (this.side === "right" && this.showId()) return this.showView();
    const t = theme();
    if (this.side === "left") return v_flex().id("left-root").size_full().bg(t.chrome).text_size(13)
      .on_size_change((size) => { this.leftH = size.height; })
      .child(v_flex().flex_1().min_h(0).overflow_y_scrollbar().children(this.left()))
      .child(this.newSpaceBar(t));
    let body;
    if (this.tab === "prs") body = this.prsBody(t);
    else if (this.tab === "deploys") body = this.deploys.length ? this.monitorBody(t, this.deploys, "d", "dp") : [empty(t, "⇪", "No deployments seen", "The monitor found none. If CircleCI rejects its token (401), set a new one:  bhote config set CIRCLECI_TOKEN <token>  . bhote monitor detect  finds a project's CI.")];
    else if (this.tab === "tests") {
      const { shown, archived } = this.testsShown();
      body = shown.length ? this.monitorBody(t, shown, "t", "ts") : (archived.length ? [] : [empty(t, "◎", "No test runs seen", "CI runs and  bhote test run -- <command>  show up here")]);
      // the toolbar: select several runs and archive them, or archive every run that is shown
      if (shown.length) {
        const btn = (id, label, onClick, o = {}) => div().id(id).px(10).py(4).rounded(6).text_size(12).cursor_pointer()
          .bg(o.primary ? t.solid : t.raised).border(1).border_color(o.primary ? t.solid : t.border).text_color(o.primary ? t.solid_foreground : t.foreground)
          .opacity(o.dim ? 0.5 : 1).hover((e) => e.bg(o.primary ? t.solid : t.hover)).on_click(this.guard(onClick)).child(label);
        const archive = (keys) => {
          const h = this.hidden();
          for (const k of keys) h.archived.add(k);
          setShared("bhote.hidden", { archived: [...h.archived].slice(-500), discarded: [...h.discarded].slice(-1000) });
        };
        const keys = shown.map((x) => this.runKey(x));
        const n = keys.filter((k) => this.selected.has(k)).length;
        body.unshift(h_flex().justify_end().gap(6).px(10).pt(8).flex_wrap()
          .when(!this.selecting, (el) => el.child(btn("sel-on", "Select", (_e, cx) => { this.selecting = true; cx.notify(); }))
            .child(btn("arch-all", "Mark all as archived", () => archive(keys))))
          .when(this.selecting, (el) => el.child(btn("sel-all", n === keys.length ? "Select none" : "Select all", (_e, cx) => { this.selected = n === keys.length ? new Set() : new Set(keys); cx.notify(); }))
            .child(btn("sel-archive", `Archive selected${n ? ` (${n})` : ""}`, (_e, cx) => { if (n) { archive(keys.filter((k) => this.selected.has(k))); this.selected = new Set(); this.selecting = false; cx.notify(); } }, { primary: true, dim: !n }))
            .child(btn("sel-off", "Cancel", (_e, cx) => { this.selecting = false; this.selected = new Set(); cx.notify(); }))));
      }
    }
    else body = this.right();
    return v_flex().size_full().bg(t.chrome).text_size(13)
      .child(this.tabBar(t))
      .child(v_flex().flex_1().min_h(0).overflow_y_scrollbar().children(body))
      .children(this.tab === "tests" ? this.archiveDock(t) : []);
  }
}
