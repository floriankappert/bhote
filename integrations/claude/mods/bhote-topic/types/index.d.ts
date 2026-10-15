export type Topic = { id: string; n: number; title: string; description: string | null; status: string; updated: number; waitingFor: string | null }

declare module 'claude-code' {
  interface PluginState {
    'bhote-topic': { topic: Topic | null; isHidden: boolean }
  }
}
