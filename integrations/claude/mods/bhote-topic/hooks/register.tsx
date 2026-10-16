import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { Progress, Topic } from '../types'

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
const cap = (x: string) => x.charAt(0).toUpperCase() + x.slice(1)
const DEPLOY_STATE: Record<string, string> = { queued: 'queued', approval: 'needs approval', ok: 'deployed', failed: 'failed', canceled: 'canceled' }
const isBusy = (p: Progress) => p.tests.length > 0 || (p.deploy !== null && ['running', 'queued', 'approval'].includes(p.deploy.state))

const parse = (stdout: string): Topic | null => {
  try {
    const t = JSON.parse(stdout)
    return t && t.id ? { id: t.id, n: t.n, title: t.title, description: t.description ?? null, status: t.status, updated: t.updated ?? 0, waitingFor: t.waiting_for ?? null, progress: t.progress ?? { tests: [], deploy: null } } : null
  } catch {
    return null
  }
}

// a turn runs, or tests or a deployment: the topic is read every few seconds
let isWorking = false
let isBusyNow = false
let tick = 0

async function refresh($: EngineInterface) {
  try {
    const ran = await $.process.run(['bhote', 'current', '--json'], { timeoutMs: 5000 })
    const next = ran.exitCode === 0 ? parse(ran.stdout) : null
    isBusyNow = next !== null && isBusy(next.progress)
    await update($, topic, () => next)
  } catch {
    await update($, topic, () => null)
  }
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'topic',
      description: 'Your bhote topic: /topic [review|done|now|next|later|testing|dismiss|wait <Name>|hide|show]',
    })
    await refresh($)
    // the "3min ago" runs on between turns
    // and a running suite or deployment moves its bar: every 5 s while a turn runs or something is under way, else once a minute
    $.clock.every(5000, async () => {
      tick += 1
      if (isWorking || isBusyNow || tick % 12 === 0) await refresh($)
      $.ui.invalidate('ui.render')
    })

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
      return { text: 'Usage: /topic [review|done|now|next|later|testing|dismiss|wait <Name>|hide|show]' }
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
            <Text bold>
              🐕 {current.title}
            </Text>
            {current.description && (
              <Box paddingLeft={3}>
                <Text dimColor>{current.description}</Text>
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
}
