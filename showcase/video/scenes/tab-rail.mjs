import { agentsAtWork } from "../../scenes.mjs";
import { flipScope, startChats, swipeTabs } from "../parts.mjs";

const row = (title) => ({
  selector: ".tab-rail .agent-row *",
  text: title,
  exact: false,
});

export default {
  setup: async (s) => {
    await startChats(s, [
      ["s-sikemux", "w-agent-phone"],
      ["s-sikemux", "w-agent-replay"],
      ["s-sikemux", "w-tab-rail"],
    ]);
    await agentsAtWork(s.appFrame);
    await s.place({ x: 960, y: 640 });
  },
  play: async (s) => {
    s.live({ wait: false });
    await s.wait(500);
    await swipeTabs(s);
    await s.wait(500);
    await swipeTabs(s);
    await s.wait(500);
    await swipeTabs(s, { pixels: -520 });
    await s.wait(500);
    await s.zoom(railHead, { scale: 2.2, at: { x: 0.5, y: 4.5 } });
    await swipeRail(s);
    await s.wait(300);
    await flipScope(s, "All projects");
    await s.wait(1300);
    await s.move(row("Fix the cover buttons"));
    await s.wait(600);
    await s.zoomOut({ wait: false });
    await s.click(row("Fix the cover buttons"));
    await s.wait(1200);
    await s.zoom(railHead, { scale: 2.2, at: { x: 0.5, y: 4.5 }, wait: false });
    await flipScope(s, "moodboard-studio");
    await s.wait(1300);
    await s.zoomOut();
    await s.wait(800);
  },
};
