import {
  browserCheck,
  composer,
  startChats,
  WREN,
  wrenAtItsDesk,
} from "../parts.mjs";

export default {
  setup: async (s) => {
    await wrenAtItsDesk(s);
    await startChats(s, [["s-mood", "w-agent-hero"]]);
    await s.place({ x: 760, y: 700 });
  },
  play: async (s) => {
    await s.wait(500);
    await s.zoom(composer, { scale: 1.6, wait: false });
    await s.click(composer);
    await s.type("Open the dev server in your tab and show me Shop the drop.");
    await s.wait(300);
    await s.key("Enter");
    await s.app(() => showcase.backend.cue("wren-checks-the-cover"));
    await s.zoomOut({ duration: 900, wait: false });
    await s.live({ agent: WREN, steps: 2 });
    await browserCheck(s);
    await s.wait(1800);
    await s.zoomOut({ duration: 1300 });
    await s.wait(900);
  },
};
