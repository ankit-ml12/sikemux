import { agentsAtWork } from "../../scenes.mjs";
import { flipScope, startChats, swipeRail, swipeTabs } from "../parts.mjs";

const railHead = ".tab-rail .swipe-page.is-current .scope-track";

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
    await s.wait(1200);
    await swipeTabs(s);
    await s.wait(1200);
    await swipeTabs(s);
    await s.wait(1200);
    await swipeTabs(s, { pixels: -600 });
    await s.wait(1000);
    await s.zoom(railHead, { scale: 2.2, at: { x: 0.5, y: 4.5 }, wait: false });
    await s.move(railHead, { at: { x: 0.6, y: 3 } });
    await swipeRail(s, { hold: 1500 });
    await s.wait(400);
    await flipScope(s, "All projects");
    await s.wait(1800);
    await s.move(row("Fix the cover buttons"));
    await s.wait(600);
    await s.zoomOut({ wait: false });
    await s.click(row("Fix the cover buttons"));
    await s.wait(1500);
    await s.zoom(railHead, { scale: 2.2, at: { x: 0.5, y: 4.5 }, wait: false });
    await flipScope(s, "moodboard-studio");
    await s.wait(1800);
    await s.zoomOut();
    await s.wait(600);
    await s.endCard();
  },
};
