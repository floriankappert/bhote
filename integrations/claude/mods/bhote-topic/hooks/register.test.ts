import { test, expect } from 'claude-code/testing'
import { register } from './register'

test('/topic without a topic says so', async ($: any, on: any) => {
  register(on, {})
  const ran = await $.command.run({ command: 'topic', args: 'review' })
  expect(JSON.stringify(ran)).toContain('No bhote topic')
})

test('/topic pane opens the pane', async ($: any, on: any) => {
  register(on, {})
  const asked: string[] = []
  on('process.run', async () => ({ value: { exitCode: 1, stdout: '', stderr: '' } }))
  on('ui.open', async () => { asked.push('open'); return { value: { isPlaced: true } } })
  const first = await $.command.run({ command: 'topic', args: 'pane' })
  expect(JSON.stringify(first)).toContain('Topic pane opened')
  expect(asked.join(',')).toBe('open')
})

test('/topic remind shows a reminder from bhote reminder', async ($: any, on: any) => {
  register(on, {})
  on('clock.now', async () => ({ value: 1000000 }))
  on('process.run', async () => ({ value: { exitCode: 0, stdout: '{"id":"1-1","title":"Call Anna","description":"about the contract","status":"waiting","waiting_for":"Anna"}', stderr: '' } }))
  const ran = await $.command.run({ command: 'topic', args: 'remind' })
  expect(JSON.stringify(ran)).toContain('Friendly reminder shown')
})

test('/topic remind says so when nothing waits', async ($: any, on: any) => {
  register(on, {})
  on('process.run', async () => ({ value: { exitCode: 0, stdout: '{}', stderr: '' } }))
  const ran = await $.command.run({ command: 'topic', args: 'remind' })
  expect(JSON.stringify(ran)).toContain('No reminder')
})
