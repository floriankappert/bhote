export type Suite = { suite: string; done: number; total: number }
export type Deploy = { project: string; label: string; state: string; done: number | null; total: number | null; url: string | null }
export type Progress = { tests: Suite[]; deploy: Deploy | null }

export type Pr = { number: number; title: string; state: string; draft: boolean; url: string; checks: { total: number; success: number; failed: number; pending: number } }
export type Sibling = { n: number; title: string; status: string }
export type Listed = { updated: number; n: number; title: string; agent: string; code?: string }
export type Lists = Record<string, { total: number; items: Listed[] }>
export type Context = { pr: Pr | null; siblings: Sibling[]; lists?: Lists }
export type ReviewAction = { label: string; reply: string | null; jump: boolean }

export type Topic = {
  id: string
  n: number
  title: string
  description: string | null
  status: string
  created: number
  updated: number
  waitingFor: string | null
  waitingSince: number | null
  project: string | null
  agent: { name: string | null; machine: string | null; branch: string | null } | null
  slack: { label: string | null; message: string | null } | null
  review: { summary: string | null; actions: ReviewAction[] } | null
  progress: Progress
  context: Context | null
}

declare module 'claude-code' {
  interface PluginState {
    'bhote-topic': { topic: Topic | null; isHidden: boolean }
  }
}
