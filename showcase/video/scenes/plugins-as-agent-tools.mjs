const MOSS = "agent-incident";
const tools = { selector: ".chat-tools", last: true };

export default {
  setup: async (s) => {
    await s.open("s-billing", "w-agent-incident");
    await s.settle(700);
    await s.place({ x: 1180, y: 720 });
  },
  play: async (s) => {
    await s.wait(1200);
    await s.live({ agent: MOSS, steps: 2, pace: 0.6 });
    s.live({ agent: MOSS, steps: 2, wait: false });
    await s.appear(tools);
    await s.zoom(tools, { scale: 2, at: { x: 0.35, y: 0.5 } });
    await s.wait(1600);
    await s.live({ agent: MOSS, steps: 1 });
    await s.zoom({ selector: ".chat-message", last: true }, { scale: 1.6 });
    await s.wait(2000);
    await s.click({ selector: ".chat-tools-sum", last: true });
    await s.click({
      selector: ".chat-tools-body button",
      text: "billing-service",
      exact: false,
      last: true,
    });
    await s.zoom(
      { selector: ".chat-tool-terminal", last: true },
      { scale: 1.9, wait: false },
    );
    await s.move(
      { selector: ".chat-tool-terminal", last: true },
      { at: { x: 0.85, y: 1.6 } },
    );
    await s.wait(2500);
    s.live({ agent: MOSS, pace: 0.6, wait: false });
    await s.zoomOut();
    await s.wait(3200);
    await s.zoom(tools, {
      scale: 1.8,
      at: { x: 0.35, y: 0.8 },
    });
    await s.wait(2000);
    await s.zoomOut();
    await s.wait(600);
    await s.endCard();
  },
};
