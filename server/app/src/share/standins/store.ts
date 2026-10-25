/*
 * Stands in for the Mac app's store when its chat view runs in a browser. The view reads which
 * plugins are switched off and whether faces are on; no plugin runs and faces stay off here.
 */
const state = {
  disabledPlugins: [] as string[],
  pluginManifests: [] as unknown[],
  faceStyle: { on: false, pack: "", shape: "", motion: false },
  faces: {} as Record<string, never>,
  agents: {} as Record<string, never>,
  agentActivity: {} as Record<string, never>,
};

export function getState() {
  return state;
}

export function useStore<T>(select: (current: typeof state) => T): T {
  return select(state);
}

export function mutate(): void {}
