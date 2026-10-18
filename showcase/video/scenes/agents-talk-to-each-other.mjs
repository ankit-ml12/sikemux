import { askOtter, composer, messageAndBoard, startChats } from "../parts.mjs";

const card = (title) => ({
  selector: ".kanban-card",
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
    await s.place(composer, { x: 0.62, y: 0.45 });
  },
  play: async (s) => {
    await s.wait(900);
    s.zoom(composer, { scale: 1.5, at: { x: 0.5, y: 0.1 } });
    await askOtter(s);
    await s.app(() => showcase.backend.cue("otter-asks-maple"));
    await s.live({ agent: "tab-rail", steps: 2 });
    await messageAndBoard(s, { hold: 1600 });
    await s.app(() => showcase.backend.cue("otter-takes-refunds"));
    await s.zoom(".kanban-columns", {
      scale: 1.2,
      at: { x: 0.5, y: 0.55 },
      wait: false,
    });
    await s.live({ agent: "tab-rail", pace: 0.3 });
    await s.zoom(card("Totals are off by a cent"), { scale: 1.8, wait: false });
    await s.move(card("Totals are off by a cent"));
    await s.wait(1200);
    await s.zoomOut();
    await s.wait(600);
  },
};
