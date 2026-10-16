export type Suite = { suite: string; done: number; total: number }
export type Deploy = { project: string; label: string; state: string; done: number | null; total: number | null; url: string | null }
export type Progress = { tests: Suite[]; deploy: Deploy | null }

export type Topic = { id: string; n: number; title: string; description: string | null; status: string; updated: number; waitingFor: string | null; progress: Progress }

declare module 'claude-code' {
  interface PluginState {
    'bhote-topic': { topic: Topic | null; isHidden: boolean }
  }
}
