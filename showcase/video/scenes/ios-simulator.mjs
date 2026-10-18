import { composer, startChats } from "../parts.mjs";

const COMET = "agent-phone";

export default {
  setup: async (s) => {
    await s.app(() => {
      showcase.backend.simulatorScreen = "terminals";
      showcase.cmd.openDeskSimulator("agent-phone", {
        focus: false,
        device: {
          udid: "5C1A0E77-8D2F-4B1E-9F63-2A7D41C0B9E8",
          name: "iPhone 18 Pro",
        },
      });
    });
    await startChats(s, [["s-sikemux", "w-agent-phone"]]);
    await s.settle(1500);
    await s.place({ x: 760, y: 700 });
  },
  play: async (s) => {
    await s.wait(500);
    await s.zoom(composer, { scale: 1.6, wait: false });
    await s.click(composer);
    await s.type(
      "Install it on the simulator and show me the permission card.",
    );
    await s.wait(300);
    await s.key("Enter");
    await s.app(() => showcase.backend.cue("comet-checks-the-card"));
    await s.zoomOut({ wait: false });
    await s.live({ agent: COMET, steps: 3 });
    await s.zoom(".sim-device", {
      scale: 1.7,
      at: { x: 0.5, y: 0.45 },
      wait: false,
    });
    await s.live({ agent: COMET, steps: 3 });
    await s.wait(900);
    await s.live({ agent: COMET, steps: 3, pace: 0.6 });
    await s.zoom({ selector: ".chat-image", last: true }, { scale: 1.5 });
    s.live({ agent: COMET, wait: false });
    await s.wait(1800);
    await s.zoomOut();
    await s.wait(800);
  },
};
