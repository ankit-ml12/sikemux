import { agentsAtWork } from "../../scenes.mjs";
import {
  approveFromNotch,
  askOtter,
  browserCheck,
  composer,
  flipScope,
  MAPLE,
  messageAndBoard,
  OTTER,
  openProject,
  startChats,
  swipeTabs,
  WREN,
  wrenAtItsDesk,
} from "../parts.mjs";

export default {
  setup: async (s) => {
    await wrenAtItsDesk(s);
    await startChats(s, [
      ["s-sikemux", "w-agent-phone"],
      ["s-sikemux", "w-agent-replay"],
      ["s-mood", "w-agent-hero"],
      ["s-sikemux", "w-tab-rail"],
    ]);
    await agentsAtWork(s.appFrame);
    await s.app(async () => {
      await showcase.backend.cue("wren-is-asked-to-check-the-cover");
      await showcase.backend.cue("otter-asks-maple");
    });
    await s.live({ agent: WREN, steps: 3 });
    await s.place(composer, { x: 0.62, y: 0.45 });
  },
  play: async (s) => {
    await s.zoom(composer, {
      scale: 1.22,
      at: { x: 0.5, y: 0.1 },
      duration: 3000,
      wait: false,
    });
    await askOtter(s, { perSecond: 26 });
    await s.live({ agent: OTTER, steps: 2, wait: false });

    await s.zoomOut({ duration: 900, wait: false });
    await swipeTabs(s);
    await s.wait(250);
    await swipeTabs(s);
    await s.wait(250);
    await openProject(s, "moodboard-studio");
    await s.zoom(".tab-rail", {
      scale: 1.8,
      at: { x: 0.5, y: 0.3 },
      duration: 900,
      wait: false,
    });
    await flipScope(s, "All projects");
    await s.wait(500);

    await browserCheck(s, { pace: 0.6 });
    await s.wait(300);

    await s.zoomOut({ duration: 800, wait: false });
    await s.click({
      selector: ".tab-rail .agent-row *",
      text: "Line up the sidebar labels",
      exact: false,
    });
    await messageAndBoard(s, { pace: 0.6, hold: 1000 });

    await s.app(() => showcase.backend.settleLive("tab-rail"));
    await s.cut(() => s.open("s-sikemux", "w-tab-rail"), { duration: 420 });
    await s.zoomOut({ duration: 10 });
    await approveFromNotch(s, { hold: 900 });

    s.live({ agent: MAPLE, wait: false });
    await s.zoomOut({ duration: 1300, wait: false });
    await s.move({ x: 1180, y: 700 });
    await s.wait(1500);
  },
};
