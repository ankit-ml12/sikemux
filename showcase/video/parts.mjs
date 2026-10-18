/*
 * Beats more than one video plays, so a post and the launch reel are cuts of
 * the same moments. Each takes the director and leaves the camera where it
 * says, for whatever plays next.
 */

export const OTTER = "tab-rail";
export const MAPLE = "agent-replay";
export const WREN = "agent-hero";

export const composer = '[role="textbox"][aria-label="Message agent"]';
const scope = (name) => ({
  text: name,
  selector: ".swipe-page.is-current .scope-track button",
  exact: false,
});
const card = (title) => ({
  selector: ".kanban-card",
  text: title,
  exact: false,
});

/** Opens each chat a video visits, so its agent has started, then leaves every chat idle. */
export async function startChats(s, windows) {
  for (const [session, window] of windows) {
    await s.open(session, window);
    await s.settle(700);
  }
  await s.app(() => showcase.backend.settleLive());
}

/** The person asks Otter to pass work on to Maple, tagging her with @. */
export async function askOtter(s, { perSecond = 15 } = {}) {
  await s.click(composer);
  await s.type("When @Ma", { perSecond });
  await s.wait(420);
  await s.key("Enter", { after: 120 });
  await s.type("lands the ledger import, take the refunds bug.", { perSecond });
  await s.wait(250);
  await s.key("Enter");
}

/** Swipes the middle of the window across to the session's next tab, or back with a negative `pixels`. */
export const swipeTabs = (s, { pixels = 520, duration = 340 } = {}) =>
  s.swipe(".window-area", { pixels, duration, at: { x: 0.5, y: 0.55 } });

export const flipScope = (s, name, options) => s.click(scope(name), options);

/** Picks a project in the side rail, which swaps the whole workspace to it. */
export const openProject = (s, name, options) =>
  s.click({ selector: ".side-rail .proj-row *", text: name }, options);

/** Wren's desk open beside its chat, on a new tab, before it is asked to look at anything. */
export async function wrenAtItsDesk(s) {
  await s.app(async () => {
    showcase.cmd.openDesk("agent-hero", { focus: false });
    const tabs = await import("/showcase/browserPage.ts");
    tabs.showBrowserPage("agent-hero", {
      url: "about:blank",
      title: "New tab",
      picture: "",
    });
  });
}

/** Wren opens the app in its tab, clicks through to the shop, and its screenshot lands in the chat. */
export async function browserCheck(s, { pace = 1 } = {}) {
  await s.zoom(".browser-viewport", {
    scale: 1.75,
    at: { x: 0.5, y: 0.42 },
    wait: false,
  });
  await s.live({ agent: WREN, steps: 2, pace });
  await s.live({ agent: WREN, steps: 3, pace: pace * 0.5 });
  await s.zoom(
    { selector: ".chat-image", last: true },
    { scale: 1.6, duration: 900 },
  );
  await s.live({ agent: WREN, wait: false });
}

/** Otter messages Maple, Maple answers from her own chat, and the card moves on the project's board. */
export async function messageAndBoard(s, { pace = 1, hold = 1400 } = {}) {
  await s.live({ agent: OTTER, steps: 1, pace: pace * 0.25 });
  await s.zoom(
    { selector: ".chat-tools", last: true },
    { scale: 1.8, at: { x: 0.42, y: 0.5 }, duration: 900 },
  );
  await s.wait(hold);
  await s.cut(() => s.open("s-sikemux", "w-agent-replay"), { duration: 420 });
  await s.app(() => showcase.backend.cue("maple-answers"));
  await s.live({ agent: MAPLE, steps: 2, pace });
  await s.zoom(
    { selector: ".agent-letter", last: true },
    { scale: 1.9, duration: 800 },
  );
  await s.wait(hold);
  await s.live({ agent: MAPLE, steps: 2, pace: pace * 0.3 });
  await s.cut(() => s.app(() => showcase.cmd.openBoard()), { duration: 420 });
  await s.appear(card("Ledger import onto amounts"));
  await s.zoom(".kanban-columns", {
    scale: 1.45,
    at: { x: 0.5, y: 0.35 },
    duration: 700,
    wait: false,
  });
  await s.move(card("Ledger import onto amounts"));
  await s.live({ agent: MAPLE, steps: 1, pace });
  await s.wait(hold);
}

const ALLOW = { stage: "img.notch" };

/** Otter asks to run a command, the island drops from the notch to ask, and the person allows it there. */
export async function approveFromNotch(s, { hold = 1300, showCard = 0 } = {}) {
  await s.app(() => showcase.backend.cue("otter-runs-the-rail-tests"));
  await s.live({ agent: OTTER, steps: 3, pace: 0.5 });
  if (showCard) {
    await s.zoom(".chat-permission", { scale: 1.7, duration: 900 });
    await s.wait(showCard);
  }
  await s.zoom(
    { stage: "img.notch" },
    { scale: 3.1, at: { x: 0.5, y: 0.2 }, duration: 1100, wait: false },
  );
  await s.island("P1-permission");
  await s.still();
  s.cursor.shape = "hand";
  await s.click(ALLOW, { at: { x: 0.685, y: 0.56 }, pause: 160 });
  s.cursor.shape = null;
  await s.island("C2-working");
  await s.live({ agent: OTTER, steps: 2, pace: 0.6 });
  await s.island("P2-finished");
  await s.wait(hold);
  await s.island("C4b-working-and-done");
  await s.live({ agent: OTTER, wait: false });
}
