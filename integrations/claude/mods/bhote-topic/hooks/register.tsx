import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { Progress, Topic } from '../types'

const PANE = 'bhote-topic'

const topic = atom({ plugin: 'bhote-topic', key: 'topic' } as const, null)
const isHidden = atom({ plugin: 'bhote-topic', key: 'isHidden' } as const, false)

// /topic <sub>: the bhote status commands that act on the topic of this agent
const STATUS = ['now', 'testing', 'next', 'review', 'later', 'done', 'dismiss']

// one theme colour per status, each its own (waiting overrides the status)
const COLOR: Record<string, string> = {
  now: 'claude',
  testing: 'permission',
  next: 'suggestion',
  review: 'error',
  later: 'inactive',
  done: 'success',
}
const colorOf = (t: Topic) => (t.waitingFor ? 'warning' : COLOR[t.status] ?? 'subtle')

const ago = (seconds: number) => {
  const m = Math.floor(Math.max(0, seconds) / 60)
  if (m < 1) return 'just now'
  if (m < 60) return `${m}min ago`
  const h = Math.floor(m / 60)
  return h < 24 ? `${h}h ago` : `${Math.floor(h / 24)}d ago`
}

const BAR = 5 // ■■■■■ as in the bhote panel and the Claude status line
const bar = (done: number, total: number) => {
  const full = total > 0 ? Math.min(BAR, Math.round((done / total) * BAR)) : 0
  return { full, rest: BAR - full, pct: total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0 }
}
// the cards of the panel, in its order: what waits for you, what runs, what waits for a deployment, what was finished lately
const SECTIONS: { key: string; title: string; color: string; mark: string }[] = [
  { key: 'review', title: 'In review', color: 'error', mark: '●' },
  { key: 'now', title: 'Now', color: 'claude', mark: '●' },
  { key: 'testing', title: 'Testing', color: 'permission', mark: '●' },
  { key: 'deploying', title: 'Deploying', color: 'warning', mark: '●' },
  { key: 'finished', title: 'Recently finished', color: 'success', mark: '✓' },
]
// the buttons of the pane: what makes sense for the status (bhote's own topic page offers the same first): a topic that runs goes to review or is done,
// one that waits for you goes on again, is closed or done, a finished card is closed or goes on, a wait that is over goes on, a queued topic starts
type Action = { label: string; cmd: string; color: string }
const RESUME: Action = { label: 'Resume', cmd: 'now', color: 'claude' }
const REVIEW: Action = { label: 'Review', cmd: 'review', color: 'error' }
const DONE: Action = { label: 'Done', cmd: 'done', color: 'success' }
const DISMISS: Action = { label: 'Dismiss', cmd: 'dismiss', color: 'inactive' }
const LATER: Action = { label: 'Later', cmd: 'later', color: 'inactive' }
const ACTIONS: Record<string, Action[]> = {
  now: [REVIEW, DONE, LATER],
  testing: [REVIEW, DONE, LATER],
  review: [RESUME, DISMISS, DONE],
  finished: [DISMISS, DONE, RESUME],
  waiting: [RESUME, DONE, LATER],
  next: [{ ...RESUME, label: 'Start' }, DONE],
  later: [{ ...RESUME, label: 'Start' }, DONE],
}
const clip = (x: string, n: number) => (x.length > n ? x.slice(0, Math.max(1, n - 1)) + '…' : x) // a Button draws its text whole: the title is cut here
const cap = (x: string) => x.charAt(0).toUpperCase() + x.slice(1)
const DEPLOY_STATE: Record<string, string> = { queued: 'queued', approval: 'needs approval', ok: 'deployed', failed: 'failed', canceled: 'canceled' }
const isBusy = (p: Progress) => p.tests.length > 0 || (p.deploy !== null && ['running', 'queued', 'approval'].includes(p.deploy.state))

const parse = (stdout: string): Topic | null => {
  try {
    const t = JSON.parse(stdout)
    if (!t || !t.id) return null
    return {
      id: t.id,
      n: t.n,
      title: t.title,
      description: t.description ?? null,
      status: t.status,
      created: t.created ?? 0,
      updated: t.updated ?? 0,
      waitingFor: t.waiting_for ?? null,
      waitingSince: t.waiting_since ?? null,
      project: t.project ?? null,
      agent: t.agent ? { name: t.agent.name ?? null, machine: t.agent.machine ?? null, branch: t.agent.branch ?? null, pane: t.agent.pane ?? null } : null,
      slack: t.slack ? { label: t.slack.label ?? null, message: t.slack.message ?? null } : null,
      review: t.review ? { summary: t.review.summary ?? null, actions: t.review.actions ?? [] } : null,
      progress: t.progress ?? { tests: [], deploy: null },
      context: t.context ?? null,
    }
  } catch {
    return null
  }
}

// a turn runs, or tests or a deployment: the topic is read every few seconds
let isWorking = false
let isBusyNow = false
let isPaneOpen = false // the pane is open: the whole picture (pull request, other topics) is read
let tick = 0

async function refresh($: EngineInterface) {
  try {
    const ran = await $.process.run(isPaneOpen ? ['bhote', 'current', '--json', '--full'] : ['bhote', 'current', '--json'], { timeoutMs: isPaneOpen ? 20000 : 5000 })
    const next = ran.exitCode === 0 ? parse(ran.stdout) : null
    isBusyNow = next !== null && isBusy(next.progress)
    await update($, topic, () => next)
  } catch {
    await update($, topic, () => null)
  }
}

export const register: Register = (on, options) => {
  const paneBg = String(options.background ?? '#11111a')

  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'topic',
      description: 'Your bhote topic: /topic [pane|review|done|now|next|later|testing|dismiss|wait <Name>|hide|show]',
    })
    await refresh($)
    // the "3min ago" runs on between turns
    // and a running suite or deployment moves its bar: every 5 s while a turn runs or something is under way, else once a minute
    $.clock.every(5000, async () => {
      tick += 1
      if (isWorking || isBusyNow || (isPaneOpen && tick % 2 === 0) || tick % 12 === 0) await refresh($)
      $.ui.invalidate('ui.render')
    })

    return next(e)
  })

  // the person closes the pane with the engine's mark (or ctrl+x x): the topic is read the light way again
  on('ui.close', async ($, e, next) => {
    if (e.id === PANE) isPaneOpen = false

    return next(e)
  })

  on('prompt.submit', async ($, e, next) => {
    isWorking = true

    return next(e)
  })

  on('turn.complete', async ($, e, next) => {
    isWorking = false
    await refresh($)

    return next(e)
  })

  on('command.run', { command: 'topic' }, async ($, e) => {
    const [sub = 'status', ...rest] = e.args.trim().split(/\s+/)
    const current = await read($, topic)

    if (sub === 'pane') {
      if (isPaneOpen) {
        await $.ui.close({ id: PANE })
        isPaneOpen = false
        return { text: 'Topic pane closed.' }
      }
      isPaneOpen = true
      await refresh($)
      const opened = await $.ui.open({ id: PANE, title: 'bhote', columns: 46, closeOnEscape: true })
      return { text: opened.isPlaced ? 'Topic pane opened (/topic pane closes it).' : 'Topic pane: it is drawn as soon as the terminal is 110 columns wide.' }
    }
    if (sub === 'hide' || sub === 'show') {
      await update($, isHidden, () => sub === 'hide')
      return { text: sub === 'hide' ? 'Topic band hidden.' : 'Topic band shown.' }
    }
    if (current === null) {
      return { text: 'No bhote topic for this agent.' }
    }
    if (sub === '' || sub === 'status') {
      return { text: `#${current.n} ${current.title} (${current.status})` }
    }

    let argv: string[]
    if (STATUS.includes(sub)) {
      argv = ['bhote', sub, current.id]
    } else if (sub === 'wait' && rest.length > 0) {
      argv = ['bhote', 'wait', current.id, rest.join(' ')]
    } else {
      return { text: 'Usage: /topic [pane|review|done|now|next|later|testing|dismiss|wait <Name>|hide|show]' }
    }

    const ran = await $.process.run(argv, { timeoutMs: 10000 })
    await refresh($)

    return { text: ran.exitCode === 0 ? `#${current.n}: ${argv.slice(1, 2).join('')} ok` : `bhote failed: ${ran.stderr.trim()}` }
  })

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const current = await read($, topic)

    if (e.props.hasSurvey || current === null || (await read($, isHidden))) {
      return next(e)
    }

    const { Box, Text } = $.ui.resolve(e)
    const since = current.updated ? ago((await $.clock.now()) / 1000 - current.updated) : ''
    const color = colorOf(current)
    const label = current.waitingFor ? `waiting for ${current.waitingFor}` : current.status

    const { tests, deploy } = current.progress
    const deployUnder = deploy !== null && ['running', 'queued', 'approval'].includes(deploy.state)
    const rows = []
    for (const t of tests) {
      const b = bar(t.done, t.total)
      rows.push(
        <Text key={'t-' + t.suite}>
          <Text dimColor>{cap(t.suite) + ' '}</Text>
          <Text color="permission">{'■'.repeat(b.full)}</Text>
          <Text dimColor>{'■'.repeat(b.rest)}</Text>
          <Text bold>{` ${b.pct}%`}</Text>
          <Text dimColor>{` (${t.done}/${t.total})`}</Text>
        </Text>,
      )
    }
    if (deploy !== null && (deployUnder || deploy.state === 'failed')) {
      const b = bar(deploy.done ?? 0, deploy.total ?? 0)
      const known = deploy.total !== null && deploy.total > 0
      rows.push(
        <Text key="deploy">
          <Text dimColor>{deploy.project + ' · ' + deploy.label + ' '}</Text>
          {known && deploy.state === 'running' ? (
            <Text>
              <Text color="claude">{'■'.repeat(b.full)}</Text>
              <Text dimColor>{'■'.repeat(b.rest)}</Text>
              <Text bold>{` ${b.pct}%`}</Text>
              <Text dimColor>{` (${deploy.done}/${deploy.total})`}</Text>
            </Text>
          ) : (
            <Text bold color={deploy.state === 'failed' ? 'error' : 'warning'}>
              {DEPLOY_STATE[deploy.state] ?? 'deploying'}
            </Text>
          )}
        </Text>,
      )
    }

    return (
      <Box borderStyle="round" borderColor={color} paddingX={1} flexDirection="column">
        <Box justifyContent="space-between">
          <Box flexDirection="column">
            <Text bold color="text">
              🐕 {current.title}
            </Text>
            {current.description && (
              <Box paddingLeft={3}>
                <Text color="inactive">{current.description}</Text>
              </Box>
            )}
          </Box>
          <Box flexDirection="column" alignItems="flex-end">
            <Text bold inverse color={color}>
              {` ${label.toUpperCase()} `}
            </Text>
            {since !== '' && <Text dimColor>{since}</Text>}
          </Box>
        </Box>
        {rows.length > 0 && <Box flexDirection="column" paddingLeft={3}>{rows}</Box>}
      </Box>
    )
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    // drawn means open: after /reload-plugins the module starts over while the pane stays, so the whole picture is read here, once
    if (!isPaneOpen) {
      isPaneOpen = true
      await refresh($)
    }
    const current = await read($, topic)
    const { Box, Text, Button, Markdown } = $.ui.resolve(e)

    if (current === null) {
      return (
        <Box>
          <Text dimColor>No bhote topic for this agent.</Text>
        </Box>
      )
    }

    const now = (await $.clock.now()) / 1000
    const color = colorOf(current)
    const label = current.waitingFor ? `waiting for ${current.waitingFor}` : current.status
    const ctx = current.context
    const { tests, deploy } = current.progress
    const jump = (x: { machine?: string; pane?: string }) => async () => {
      if (x.pane) await $.process.run(['bhote', 'jump', x.machine ?? '', x.pane], { timeoutMs: 15000 })
    }
    const act = (...argv: string[]) => async () => {
      await $.process.run(['bhote', ...argv, current.id], { timeoutMs: 10000 })
      await refresh($)
    }
    // a link without the URL written out: herdr's terminal draws no hyperlinks, so Link would print the whole address; this opens it in the browser
    const openUrl = (key: string, url: string) => (
      <Button
        key={key}
        plain
        onPress={async () => {
          await $.process.run(['sh', '-c', 'command -v xdg-open >/dev/null 2>&1 && xdg-open "$1" || open "$1"', 'sh', url], { timeoutMs: 10000 })
        }}
      >
        <Text dimColor underline>{' ↗ open'}</Text>
      </Button>
    )
    const line = (name: string, value: string, c?: string) => (
      <Text key={name}>
        <Text dimColor>{name.padEnd(8)}</Text>
        <Text color={c}>{value}</Text>
      </Text>
    )
    const rows = []
    if (current.project) rows.push(line('Project', current.project))
    if (current.agent) {
      rows.push(line('Agent', [current.agent.name, current.agent.machine].filter(Boolean).join(' · ')))
      if (current.agent.branch) rows.push(line('Branch', current.agent.branch))
    }
    if (current.waitingFor) rows.push(line('Waits', `for ${current.waitingFor}${current.waitingSince ? ' · ' + ago(now - current.waitingSince) : ''}`, 'warning'))
    if (current.slack) rows.push(line('Slack', (current.slack.label ?? 'pinned') + (current.status === 'waiting' ? ' (watched)' : '')))
    if (current.created) rows.push(line('Created', ago(now - current.created)))

    const bars = []
    for (const t of tests) {
      const b = bar(t.done, t.total)
      bars.push(
        <Text key={'t-' + t.suite}>
          <Text dimColor>{cap(t.suite) + ' '}</Text>
          <Text color="permission">{'■'.repeat(b.full)}</Text>
          <Text dimColor>{'■'.repeat(b.rest)}</Text>
          <Text bold>{` ${b.pct}%`}</Text>
          <Text dimColor>{` (${t.done}/${t.total})`}</Text>
        </Text>,
      )
    }
    if (deploy !== null && (['running', 'queued', 'approval'].includes(deploy.state) || deploy.state === 'failed')) {
      const b = bar(deploy.done ?? 0, deploy.total ?? 0)
      const known = deploy.total !== null && deploy.total > 0 && deploy.state === 'running'
      bars.push(
        <Box key="deploy">
          <Text>
            <Text dimColor>{deploy.project + ' · ' + deploy.label + ' '}</Text>
            {known ? (
              <Text>
                <Text color="claude">{'■'.repeat(b.full)}</Text>
                <Text dimColor>{'■'.repeat(b.rest)}</Text>
                <Text bold>{` ${b.pct}%`}</Text>
              </Text>
            ) : (
              <Text bold color={deploy.state === 'failed' ? 'error' : 'warning'}>
                {DEPLOY_STATE[deploy.state] ?? 'deploying'}
              </Text>
            )}
          </Text>
          {deploy.url && openUrl("deploy-open", deploy.url)}
        </Box>,
      )
    }

    // this session's agent as the lists name it: herdr's workspace and pane number, w3:a1 for the pane w3:p1
    const pane = current.agent?.pane ?? ''
    const thisAgent = /^w[A-Za-z0-9]+:p\d+$/.test(pane) ? pane.replace(/:p(\d+)$/, ':a$1') : ''
    const room = Math.max(10, e.props.bodyColumns - 6) // the width of an item's title: the pane's body less the padding and the mark
    const pr = ctx?.pr ?? null
    const reply = current.review?.actions.filter(a => !a.jump && a.reply) ?? []
    // a button as a chip: bold dark text on a coloured block (FINISHED), one press anywhere on it
    const chip = (key: string, text: string, c: string, onPress: () => Promise<void>) => (
      <Box key={key} marginRight={1}>
        <Button key={key} plain onPress={onPress}>
          <Text bold color="inverseText" backgroundColor={c}>{` ${text.toUpperCase()} `}</Text>
        </Button>
      </Box>
    )

    return (
      <Box flexDirection="column" paddingX={1} paddingTop={1} backgroundColor={paneBg} minHeight={e.props.scroll.bodyRows}>
        {thisAgent !== '' && (
          <Text>
            <Text dimColor>{'This agent  '}</Text>
            <Text bold color="text">{thisAgent}</Text>
          </Text>
        )}
        <Box borderStyle="round" borderColor={color} paddingX={1} flexDirection="column">
          <Box justifyContent="space-between">
            <Text bold color="text">{current.title}</Text>
          </Box>
          <Box marginBottom={1}>
            <Text bold color="inverseText" backgroundColor={color}>
              {` ${label.toUpperCase()} `}
            </Text>
            <Text dimColor>{current.updated ? '  ' + ago(now - current.updated) : ''}</Text>
          </Box>
          {current.description && <Text color="inactive">{current.description}</Text>}
          {bars.length > 0 && (
            <Box flexDirection="column" marginTop={1}>
              {bars}
            </Box>
          )}
          {current.review?.summary && (
            <Box flexDirection="column" marginTop={1}>
              <Text bold color="error">Review</Text>
              <Markdown text={current.review.summary} />
            </Box>
          )}
          {reply.length > 0 && (
            <Box marginTop={1}>
              {reply.map(a =>
                chip('a-' + a.label, a.label, 'claude', async () => {
                  await $.prompt.submit({ text: a.reply ?? a.label, asUser: true })
                  await $.process.run(['bhote', 'now', current.id], { timeoutMs: 10000 })
                  await refresh($)
                }),
              )}
            </Box>
          )}
          <Box marginTop={1}>
            {(ACTIONS[current.status] ?? ACTIONS.now).map(a => chip(a.cmd + a.label, a.label, a.color, act(a.cmd)))}
          </Box>
        </Box>
        {SECTIONS.map(sec => {
          const list = ctx?.lists?.[sec.key]
          if (!list || list.items.length === 0) return null
          return (
            <Box key={'sec-' + sec.key} flexDirection="column" marginTop={1}>
              <Text>
                <Text bold color={sec.color}>{sec.title}</Text>
                <Text dimColor>{'  ' + list.total}</Text>
              </Text>
              {list.items.map(x => (
                // two lines, both pressable: the title, and under it the elbow, the agent's address and when; a press shows the agent in herdr
                <Box key={sec.key + '-' + x.n + x.title} flexDirection="column" paddingX={1}>
                  <Button key={'t-' + x.n + x.title} plain onPress={jump(x)}>
                    <Text>
                      <Text color={sec.color}>{sec.mark + ' '}</Text>
                      <Text bold color="text">{clip(x.title, room)}</Text>
                    </Text>
                  </Button>
                  <Button key={'a-' + x.n + x.title} plain onPress={jump(x)}>
                    <Text color="inactive">{'  ∟ ' + [x.code || x.agent, ago(now - x.updated)].filter(Boolean).join('  ')}</Text>
                  </Button>
                </Box>
              ))}
              {list.total > list.items.length && <Text dimColor>{`+${list.total - list.items.length} more`}</Text>}
            </Box>
          )
        })}
        <Box flexGrow={1} />
        <Box flexDirection="column" marginTop={1}>
          {pr !== null && (
            <Box>
              <Text>
                <Text dimColor>{'PR'.padEnd(8)}</Text>
                <Text bold>{`#${pr.number}`}</Text>
                <Text color={pr.state === 'merged' ? 'permission' : pr.state === 'closed' ? 'error' : 'success'}>{` ${pr.draft ? 'draft' : pr.state} `}</Text>
                {pr.checks.total > 0 && (
                  <Text>
                    <Text color="success">{`✓${pr.checks.success} `}</Text>
                    {pr.checks.failed > 0 && <Text color="error">{`✗${pr.checks.failed} `}</Text>}
                    {pr.checks.pending > 0 && <Text color="warning">{`◌${pr.checks.pending}`}</Text>}
                  </Text>
                )}
              </Text>
              {openUrl("pr-open", pr.url)}
            </Box>
          )}
          {rows}
        </Box>
      </Box>
    )
  })
}
