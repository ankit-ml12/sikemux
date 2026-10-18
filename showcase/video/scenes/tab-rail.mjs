import { agentsAtWork } from "../../scenes.mjs";

const scope = (name) => ({ text: name, selector: ".swipe-page.is-current .scope-track button", exact: false });
const page = ".tab-rail .swipe-page.is-current";

export default {
  setup: async (s) => {
    await s.open("s-sikemux", "w-tab-rail");
    await agentsAtWork(s.appFrame);
    await s.place({ x: 1240, y: 760 });
  },
  play: async (s) => {
    s.live({ wait: false });
    await s.wait(700);
    await s.zoom(".tab-rail", { scale: 2.2, at: { x: 0.5, y: 0.3 }, wait: false });
    await s.wait(250);
    await s.click(scope("All projects"));
    await s.wait(1300);
    await s.move({ text: "Fix the cover buttons", selector: ".agent-row *" });
    await s.wait(1000);
    await s.swipe(page);
    await s.wait(1300);
    await s.click(scope("sikemux"));
    await s.wait(1200);
    await s.click(scope("All projects"));
    await s.wait(1300);
    await s.swipe(page);
    await s.wait(1500);
    await s.swipe(page, { pixels: -220 });
    await s.wait(450);
    await s.swipe(page, { pixels: -220 });
    await s.wait(1000);
    await s.zoomOut({ duration: 1300 });
    await s.wait(900);
  },
};
