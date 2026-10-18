const composer = '[role="textbox"][aria-label="Message agent"]';
const lastTools = { selector: ".chat-tools", last: true };
const card = (title) => ({
  selector: ".kanban-card",
  text: title,
  exact: false,
});

export default {
  setup: async (s) => {
    await s.open("s-sikemux", "w-agent-replay");
    await s.settle(800);
    await s.open("s-sikemux", "w-tab-rail");
    await s.settle(800);
    await s.app(() => showcase.backend.settleLive());
    await s.place({ x: 1100, y: 640 });
  },
  play: async (s) => {
    await s.wait(400);
    await s.zoom(composer, { scale: 1.7, wait: false });
    await s.click(composer);
    await s.type("When @Ma");
    await s.wait(500);
    await s.key("Enter", { after: 150 });
    await s.type("lands the ledger import, take the refunds bug.");
    await s.wait(350);
    await s.key("Enter");
    await s.zoomOut({ wait: false });
    await s.app(() => showcase.backend.cue("otter-asks-maple"));
    s.live({ wait: false });
    await s.appear(lastTools);
    await s.wait(150);
    await s.zoom(lastTools, { scale: 1.8, at: { x: 0.42, y: 0.5 } });
    await s.wait(2000);
    await s.zoomOut({ wait: false });
    await s.wait(1100);

    await s.cut(() => s.open("s-sikemux", "w-agent-replay"));
    await s.app(() => showcase.backend.cue("maple-answers"));
    await s.live({ steps: 2 });
    await s.zoom({ selector: ".agent-letter", last: true }, { scale: 1.9 });
    await s.wait(1800);
    await s.live({ steps: 2, wait: false });
    await s.zoomOut({ duration: 900 });
    await s.wait(1700);

    await s.cut(() => s.app(() => showcase.cmd.openBoard()));
    await s.wait(500);
    await s.zoom(".kanban-columns", { scale: 1.45, at: { x: 0.5, y: 0.35 } });
    await s.move(card("Ledger import onto amounts"));
    await s.wait(500);
    await s.live({ steps: 1 });
    await s.wait(900);
    await s.app(() => showcase.backend.cue("otter-takes-refunds"));
    await s.live({ pace: 0.4 });
    await s.move(card("Totals are off by a cent"));
    await s.wait(2200);
    await s.zoomOut({ duration: 1300 });
    await s.wait(800);
  },
};
