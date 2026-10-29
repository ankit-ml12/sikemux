import type { AgentFace, AgentShown, FaceStyle } from "@shared/wire.generated";
import type { Agent } from "./agent";

/* What the chat view reads of the app's state: whether faces are on, which on the page they never are. */
interface PageState {
  readonly faceStyle: FaceStyle;
  readonly faces: Readonly<Record<string, AgentFace | null>>;
  readonly agents: Readonly<Record<string, Agent>>;
  readonly agentStatus: Readonly<Record<string, AgentShown>>;
}

const state: PageState = {
  faceStyle: { on: false, pack: "", shape: "", motion: false },
  faces: {},
  agents: {},
  agentStatus: {},
};

export function useStore<T>(select: (current: PageState) => T): T {
  return select(state);
}
