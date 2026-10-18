import { agentsAtWork } from "../../scenes.mjs";
import {
  approveFromNotch,
  askOtter,
  browserCheck,
  chartAnswer,
  composer,
  flipScope,
  MAPLE,
  messageAndBoard,
  OTTER,
  openProject,
  startChats,
  swipeRail,
  swipeTabs,
  WREN,
  wrenAtItsDesk,
} from "../parts.mjs";

const railHead = ".tab-rail .swipe-page.is-current .scope-track";
const otterRow = {
  selector: ".tab-rail .agent-row *",
  text: "Line up the sidebar labels",
  exact: false,
};

export default {
  setup: async (s) => {
    await wrenAtItsDesk(s);
    await startChats(s, [
      ["s-sikemux", "w-agent-phone"],
      ["s-sikemux", "w-agent-replay"],
      ["s-mood", "w-agent-palette"],
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
    s.handPace = 0.8;
    await s.wait(900);
    s.zoom(composer, { scale: 1.22, at: { x: 0.5, y: 0.1 } });
    await askOtter(s, { perSecond: 30 });
    await s.live({ agent: OTTER, steps: 2, wait: false });

    await s.zoomOut({ wait: false });
    await swipeTabs(s);
    await swipeTabs(s);
    await s.zoom(railHead, {
      scale: 1.8,
      at: { x: 0.5, y: 4 },
      wait: false,
    });
    await s.move(railHead, { at: { x: 0.6, y: 3 } });
    await swipeRail(s, { hold: 150 });
    await s.zoomOut({ wait: false });
    await openProject(s, "moodboard-studio");
    await s.zoom(railHead, {
      scale: 1.8,
      at: { x: 0.5, y: 4 },
      wait: false,
    });
    await flipScope(s, "All projects");
    await s.wait(350);

    await browserCheck(s, { pace: 0.45 });
    await chartAnswer(s, { pace: 0.6, hold: 300 });

    await s.zoomOut({ wait: false });
    await s.click(otterRow);
    await messageAndBoard(s, { pace: 0.45, hold: 350 });

    await s.zoomOut({ wait: false });
    await s.click(otterRow);
    await approveFromNotch(s, { hold: 500 });

    s.live({ agent: MAPLE, wait: false });
    await s.zoomOut({ wait: false });
    await s.move({ x: 1180, y: 700 });
    await s.wait(500);
  },
};
