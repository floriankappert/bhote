import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { Topic } from '../types'

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

const parse = (stdout: string): Topic | null => {
  try {
    const t = JSON.parse(stdout)
    return t && t.id ? { id: t.id, n: t.n, title: t.title, description: t.description ?? null, status: t.status, updated: t.updated ?? 0, waitingFor: t.waiting_for ?? null } : null
  } catch {
    return null
  }
}

async function refresh($: EngineInterface) {
  try {
    const ran = await $.process.run(['bhote', 'current', '--json'], { timeoutMs: 5000 })
    await update($, topic, () => (ran.exitCode === 0 ? parse(ran.stdout) : null))
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
    $.clock.every(60000, () => $.ui.invalidate('ui.render'))

    return next(e)
  })

  on('turn.complete', async ($, e, next) => {
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

    return (
      <Box borderStyle="round" borderColor={color} paddingX={1} justifyContent="space-between">
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
    )
  })
}
