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
