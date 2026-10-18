import { approveFromNotch, OTTER, startChats } from "../parts.mjs";

export default {
  setup: async (s) => {
    await startChats(s, [["s-sikemux", "w-tab-rail"]]);
    await s.place({ x: 1200, y: 640 });
  },
  play: async (s) => {
    await s.wait(1200);
    await approveFromNotch(s, { showCard: 1800 });
    await s.zoomOut({ wait: false });
    await s.live({ agent: OTTER, pace: 0.7 });
    await s.zoom({ selector: ".chat-message", last: true }, { scale: 1.6 });
    await s.wait(2000);
    await s.zoomOut();
    await s.wait(600);
    await s.endCard();
  },
};
