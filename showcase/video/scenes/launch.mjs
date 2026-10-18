import { agentsAtWork } from "../../scenes.mjs";
import {
  approveFromNotch,
  askOtter,
  boardMoves,
  browserCheck,
  composer,
  mapleAnswers,
  openProject,
  otterMessagesMaple,
  startChats,
  swipeRail,
  WREN,
  wrenAtItsDesk,
} from "../parts.mjs";

const railHead = ".tab-rail .swipe-page.is-current .scope-track";

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
    await s.wait(1200);
    await s.zoom(composer, { scale: 1.3, at: { x: 0.5, y: 0.1 } });
    await askOtter(s, { perSecond: 30 });
    await otterMessagesMaple(s);
    await mapleAnswers(s);
    await boardMoves(s);

    await s.zoomOut({ wait: false });
    await s.zoom(railHead, { scale: 1.8, at: { x: 0.5, y: 4 }, wait: false });
    await s.move(railHead, { at: { x: 0.6, y: 3 } });
    await s.still();
    await swipeRail(s, { hold: 1500 });
    await s.zoomOut({ wait: false });
    await openProject(s, "moodboard-studio");
    await s.wait(500);

    await browserCheck(s);

    await s.zoomOut({ wait: false });
    await s.wait(400);
    await approveFromNotch(s);

    await s.zoomOut();
    await s.wait(600);
    await s.endCard({ hold: 2000 });
  },
};
