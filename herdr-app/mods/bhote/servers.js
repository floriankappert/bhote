#!/usr/bin/env node
// The state of the IMS dev servers of this machine as one JSON document, for the Servers tile of the bhote panel in herdr-app.
//
//   node servers.js <main checkout of IMS>
//
// What it reads, read only (it starts and stops nothing; that is what ims-start and ims-stop do):
//   - the systems: the main checkout and its worktrees `wt1`, `wt2` ... that have their own server/.env (as scripts/dev/ims-shell.zsh counts them),
//     with branch, database, the server port, and the client and portal port that follow from it
//   - pm2: which apps of those systems run (`pm2 jlist`), their pid, uptime and restarts
//   - ps: CPU and RAM of every app, with its children (the client and the portal run under `npm run dev`, vite is the child of npm)
//   - the machine: RAM, CPU, load, uptime
//   - the tunnels to other machines (IMS_TUNNELS in the main checkout's server/.env): open, or not
//
// CommonJS and no dependencies: it runs under whatever node is active, on macOS and on Linux.

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const main = process.argv[2];
if (!main) { console.error('usage: node servers.js <IMS main checkout>'); process.exit(2); }

// The node that runs this script is the one the dev shell uses: its folder goes first on the PATH, so that `pm2` and `npx` are found next to it.
process.env.PATH = [path.dirname(process.execPath), process.env.PATH, '/opt/homebrew/bin', '/usr/local/bin'].filter(Boolean).join(path.delimiter);

const run = (cmd, args, opts = {}) => {
  try { return execFileSync(cmd, args, { encoding: 'utf8', maxBuffer: 16 << 20, timeout: 15000, stdio: ['ignore', 'pipe', 'ignore'], ...opts }); } catch (_) { return ''; }
};
const read = (file) => { try { return fs.readFileSync(file, 'utf8'); } catch (_) { return ''; } };
const envValue = (dir, key) => {
  const m = new RegExp(`^${key}=(.*)$`, 'm').exec(read(path.join(dir, 'server', '.env')));
  return m ? m[1].trim() : '';
};
const isRealFile = (file) => { try { return fs.lstatSync(file).isFile(); } catch (_) { return false; } };

// ── the systems ─────────────────────────────────────────────────────────────────────────────────────────────
const mainName = path.basename(main);
const worktrees = run('git', ['-C', main, 'worktree', 'list', '--porcelain']).split('\n').filter((l) => l.startsWith('worktree ')).map((l) => l.slice(9));
const slotOf = (dir) => (dir === main ? 'main' : path.basename(dir).startsWith(`${mainName}-`) ? path.basename(dir).slice(mainName.length + 1) : path.basename(dir));
const systems = worktrees
  .filter((dir) => isRealFile(path.join(dir, 'server', '.env')))
  .map((dir) => ({ dir, slot: slotOf(dir) }))
  .filter((s) => /^(main|wt\d+)$/.test(s.slot))   // the helper worktrees of the agents are not servers to start from here
  .map(({ dir, slot }) => {
    const port = Number(envValue(dir, 'PORT')) || 3001;
    return {
      slot, dir, port, client: 5173 + (port - 3001) * 10, portal: 5175 + (port - 3001) * 10,
      branch: run('git', ['-C', dir, 'rev-parse', '--abbrev-ref', 'HEAD']).trim(), db: envValue(dir, 'DB_NAME'),
      apps: [], running: false, cpu: 0, rss: 0,
    };
  })
  .sort((a, b) => (a.slot === 'main' ? -1 : b.slot === 'main' ? 1 : a.slot.localeCompare(b.slot, undefined, { numeric: true })));

// ── ps: one call, the whole process tree ────────────────────────────────────────────────────────────────────
const procs = new Map();
const kids = new Map();
for (const line of run('ps', ['-Ao', 'pid=,ppid=,%cpu=,rss=']).split('\n')) {
  const p = line.trim().split(/\s+/);
  if (p.length < 4) continue;
  const pid = Number(p[0]);
  const ppid = Number(p[1]);
  if (!Number.isFinite(pid)) continue;
  procs.set(pid, { ppid, cpu: parseFloat(p[2].replace(',', '.')) || 0, rss: (Number(p[3]) || 0) * 1024 });
  if (!kids.has(ppid)) kids.set(ppid, []);
  kids.get(ppid).push(pid);
}
const tree = (pid) => {
  const total = { cpu: 0, rss: 0 };
  const seen = new Set();
  const walk = (id) => {
    if (seen.has(id) || !procs.has(id)) return;
    seen.add(id);
    total.cpu += procs.get(id).cpu;
    total.rss += procs.get(id).rss;
    for (const k of kids.get(id) ?? []) walk(k);
  };
  walk(pid);
  return total;
};

// ── pm2 ─────────────────────────────────────────────────────────────────────────────────────────────────────
// pm2 may write a hint before the JSON (and coloured): the document is the last [ ... ] block.
const raw = run('pm2', ['jlist']).replace(/\x1b\[[0-9;]*m/g, '');
let list = [];
try { const m = raw.match(/\[[\s\S]*\]/); if (m) list = JSON.parse(m[0]); } catch (_) { list = []; }
const now = Date.now();
for (const a of list) {
  const m = /^ims-(main|wt\d+)-(.+)$/.exec(a.name || '');
  if (!m) continue;
  const sys = systems.find((s) => s.slot === m[1]);
  if (!sys) continue;
  const online = a.pm2_env && a.pm2_env.status === 'online';
  const use = online && a.pid ? tree(a.pid) : { cpu: 0, rss: 0 };
  sys.apps.push({
    role: m[2], status: a.pm2_env ? a.pm2_env.status : 'unknown', pid: a.pid || 0, restarts: a.pm2_env ? a.pm2_env.restart_time || 0 : 0,
    up: online && a.pm2_env.pm_uptime ? Math.max(0, now - a.pm2_env.pm_uptime) : 0, cpu: use.cpu, rss: use.rss,
  });
  sys.cpu += use.cpu;
  sys.rss += use.rss;
}
for (const s of systems) { s.apps.sort((a, b) => a.role.localeCompare(b.role)); s.running = s.apps.some((a) => a.status === 'online'); }

// ── the machine ─────────────────────────────────────────────────────────────────────────────────────────────
const cores = os.cpus().length || 1;
let total = os.totalmem();
let available = os.freemem();
if (process.platform === 'linux') {
  const mem = read('/proc/meminfo');
  const kb = (k) => { const m = new RegExp(`^${k}:\\s+(\\d+)`, 'm').exec(mem); return m ? Number(m[1]) * 1024 : 0; };
  total = kb('MemTotal') || total;
  available = kb('MemAvailable') || available;
} else if (process.platform === 'darwin') {
  const vm = run('vm_stat');
  const page = Number((/page size of (\d+)/.exec(vm) || [])[1]) || 16384;
  const pages = (k) => Number((new RegExp(`${k}:\\s+(\\d+)`).exec(vm) || [])[1]) || 0;
  total = Number(run('sysctl', ['-n', 'hw.memsize']).trim()) || total;
  available = (pages('Pages free') + pages('Pages inactive') + pages('Pages speculative') + pages('Pages purgeable')) * page || available;
}
let busy = 0;
for (const p of procs.values()) busy += p.cpu;
const machine = {
  host: os.hostname().split('.')[0], platform: process.platform, cores, up: os.uptime() * 1000,
  load: os.loadavg()[0], cpu: Math.min(100, busy / cores), ramTotal: total, ramUsed: Math.max(0, total - available),
};

// ── the tunnels to other machines ───────────────────────────────────────────────────────────────────────────
const tunnels = envValue(main, 'IMS_TUNNELS').replace(/\s+/g, '').split(',').filter(Boolean).map((entry) => {
  const host = entry.split(':')[0];
  const sock = path.join(os.homedir(), '.cache', 'ims-tunnels', `${host}.sock`);
  let open = false;
  try { execFileSync('ssh', ['-S', sock, '-O', 'check', host], { stdio: 'ignore', timeout: 5000 }); open = true; } catch (_) { open = false; }
  return { entry, host, block: entry.split(':')[1] || '', open };
});

process.stdout.write(`${JSON.stringify({ at: now, machine, systems, tunnels })}\n`);
