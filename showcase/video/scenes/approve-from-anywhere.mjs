import { approveFromNotch, OTTER, startChats } from "../parts.mjs";

export default {
  setup: async (s) => {
    await startChats(s, [["s-sikemux", "w-tab-rail"]]);
    await s.place({ x: 1200, y: 640 });
  },
  play: async (s) => {
    await s.wait(700);
    await approveFromNotch(s, { hold: 1500, showCard: 1400 });
    await s.zoomOut({ duration: 1400, wait: false });
    await s.live({ agent: OTTER, pace: 0.7 });
    await s.zoom(
      { selector: ".chat-message", last: true },
      { scale: 1.6, duration: 1100 },
    );
    await s.wait(1600);
    await s.zoomOut({ duration: 1200 });
    await s.wait(700);
  },
};
