import {
  askOtter,
  boardMoves,
  composer,
  mapleAnswers,
  otterMessagesMaple,
  startChats,
} from "../parts.mjs";

export default {
  setup: async (s) => {
    await startChats(s, [
      ["s-sikemux", "w-agent-replay"],
      ["s-sikemux", "w-tab-rail"],
    ]);
    await s.app(() => showcase.backend.cue("otter-asks-maple"));
    await s.place(composer, { x: 0.62, y: 0.45 });
  },
  play: async (s) => {
    await s.wait(1200);
    await s.zoom(composer, { scale: 1.5, at: { x: 0.5, y: 0.1 } });
    await askOtter(s);
    await otterMessagesMaple(s);
    await mapleAnswers(s);
    await boardMoves(s);
    await s.zoomOut();
    await s.wait(900);
  },
};
