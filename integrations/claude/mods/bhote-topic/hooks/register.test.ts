import { test, expect } from 'claude-code/testing'
import { register } from './register'

test('/topic without a topic says so', async ($: any, on: any) => {
  register(on, {})
  const ran = await $.command.run({ command: 'topic', args: 'review' })
  expect(JSON.stringify(ran)).toContain('No bhote topic')
})
