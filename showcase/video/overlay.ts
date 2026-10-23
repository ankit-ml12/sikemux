import "./overlay.css";
import { STAGE } from "../stage.mjs";

const CENTRE = { x: STAGE.width / 2, y: STAGE.height / 2 };

/**
 * Where a beat points: a selector in the app, the first (or last) element of the
 * app matching a selector and holding some text, a part of the framed screen, or a rect on it.
 */
export type Target =
  | string
  | { selector?: string; text?: string; exact?: boolean; last?: boolean }
  | { stage: string }
  | Rect;

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Frame {
  camera: { x: number; y: number; scale: number; blur: number };
  cursor: {
    x: number;
    y: number;
    press: number;
    opacity: number;
    shape: Shape | null;
  };
}

export type Shape = "arrow" | "hand" | "beam";

const camera = document.querySelector<HTMLElement>(".camera")!;
const app = document.querySelector<HTMLIFrameElement>("iframe")!;

const ARROW = `<svg viewBox="-2 -2 22 30" width="22" height="30"><path d="M0 0v22.6l5.3-5.1 3.6 8.4 3.5-1.5-3.6-8.3H16L0 0z" fill="#000" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>`;
const HAND = `<svg viewBox="-1 -1 26 30" width="26" height="30"><path d="M8.6 1.2c-1.2 0-2.1.9-2.1 2.1v10.3l-1.8-1.9c-.9-.9-2.2-1-3.1-.2-.9.8-1 2-.3 3l4.3 6c1.6 2.2 4 3.6 6.8 3.6h2.4c3.9 0 7-3.1 7-7v-5.2c0-1.2-.9-2.1-2.1-2.1s-2.1.9-2.1 2.1v-1c0-1.2-.9-2.1-2.1-2.1s-2.1.9-2.1 2.1v-.9c0-1.2-.9-2.1-2.1-2.1s-2.1.9-2.1 2.1V3.3c0-1.2-.9-2.1-2.1-2.1z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/><path d="M12.8 15.8v3M16.9 15.8v3M21 15.8v2.6" stroke="#000" stroke-width="1.2" stroke-linecap="round"/></svg>`;
const BEAM = `<svg viewBox="-3 -2 14 28" width="14" height="28"><path d="M0 0c2.4 0 3.6.8 4 2.2C4.4.8 5.6 0 8 0M4 2.2v19.6M0 24c2.4 0 3.6-.8 4-2.2.4 1.4 1.6 2.2 4 2.2M1.6 12h4.8" fill="none" stroke="#fff" stroke-width="3.4" stroke-linecap="round"/><path d="M0 0c2.4 0 3.6.8 4 2.2C4.4.8 5.6 0 8 0M4 2.2v19.6M0 24c2.4 0 3.6-.8 4-2.2.4 1.4 1.6 2.2 4 2.2M1.6 12h4.8" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/></svg>`;

const cursor = document.createElement("div");
cursor.className = "video-cursor";
cursor.innerHTML = `<span data-shape="arrow">${ARROW}</span><span data-shape="hand">${HAND}</span><span data-shape="beam">${BEAM}</span>`;
camera.append(cursor);

const INTERACTIVE =
  "button, a[href], [role=button], [role=tab], [role=menuitem], [role=option], [role=treeitem], [role=link], summary, label, select";

function shapeAt(x: number, y: number): Shape {
  const element = app.contentDocument?.elementFromPoint(
    x - app.offsetLeft,
    y - app.offsetTop,
  );
  if (!element) return "arrow";
  const style = element.ownerDocument.defaultView!.getComputedStyle(element);
  if (
    style.cursor === "text" ||
    (style.cursor === "auto" &&
      element.closest(
        "input, textarea, [contenteditable=true], [role=textbox]",
      ))
  )
    return "beam";
  if (style.cursor === "pointer" || element.closest(INTERACTIVE)) return "hand";
  return "arrow";
}

const visible = (element: Element) => {
  const box = element.getBoundingClientRect();
  return (
    box.width > 0 &&
    box.height > 0 &&
    getComputedStyle(element).visibility !== "hidden"
  );
};

function element(target: Exclude<Target, Rect>): Element | null {
  if (typeof target === "object" && "stage" in target)
    return document.querySelector(target.stage);
  const doc = app.contentDocument!;
  if (typeof target === "string")
    return [...doc.querySelectorAll(target)].find(visible) ?? null;
  const wanted = target.text;
  const matches = (text: string) =>
    wanted === undefined ||
    (target.exact === false
      ? text.trim().includes(wanted)
      : text.trim() === wanted);
  const candidates = [...doc.querySelectorAll(target.selector ?? "*")].filter(
    (candidate) => visible(candidate) && matches(candidate.textContent ?? ""),
  );
  // The innermost match is the one the words are drawn in.
  const innermost =
    wanted === undefined
      ? candidates
      : candidates.filter(
          (candidate) =>
            !candidates.some(
              (other) => other !== candidate && candidate.contains(other),
            ),
        );
  return (target.last ? innermost.at(-1) : innermost[0]) ?? null;
}

/** A target's box on the stage, before the camera moves it. */
function rect(target: Target): Rect | null {
  if (typeof target === "object" && "width" in target) return target;
  const found = element(target);
  if (!found) return null;
  const box = found.getBoundingClientRect();
  if (found.ownerDocument !== document)
    return {
      x: box.x + app.offsetLeft,
      y: box.y + app.offsetTop,
      width: box.width,
      height: box.height,
    };
  // The screen's own parts move with the camera, so their box is taken back to where they sit unmoved.
  const { x, y, scale } = lens;
  return {
    x: (box.x - (CENTRE.x - x * scale)) / scale,
    y: (box.y - (CENTRE.y - y * scale)) / scale,
    width: box.width / scale,
    height: box.height / scale,
  };
}

function documents(): Document[] {
  const found: Document[] = [];
  const walk = (doc: Document | null | undefined) => {
    if (!doc) return;
    found.push(doc);
    for (const frame of doc.querySelectorAll("iframe")) {
      try {
        walk(frame.contentDocument);
      } catch {}
    }
  };
  walk(document);
  return found;
}

const clocks = new WeakMap<Animation, { origin: number; from: number }>();

/**
 * The renderer holds every page's timeline still, so CSS transitions and
 * animations move only here, by the same virtual time the scripts run on.
 */
function syncAnimations(now: number) {
  for (const doc of documents()) {
    for (const animation of doc.getAnimations()) {
      if (animation.playState !== "running") continue;
      let clock = clocks.get(animation);
      if (!clock) {
        clock = { origin: now, from: Number(animation.currentTime ?? 0) };
        clocks.set(animation, clock);
      }
      animation.currentTime =
        clock.from + (now - clock.origin) * animation.playbackRate;
    }
  }
}

let shape: Shape | null = null;
let lens: Frame["camera"] = { ...CENTRE, scale: 1, blur: 0 };

function show(frame: Frame, now: number) {
  lens = frame.camera;
  const { x, y, scale, blur } = frame.camera;
  camera.style.transform = `translate(${CENTRE.x - x * scale}px, ${CENTRE.y - y * scale}px) scale(${scale})`;
  camera.style.filter = blur > 0.05 ? `blur(${blur}px)` : "";
  const next = frame.cursor.shape ?? shapeAt(frame.cursor.x, frame.cursor.y);
  if (next !== shape) {
    shape = next;
    cursor.dataset.shape = next;
  }
  cursor.style.opacity = String(frame.cursor.opacity);
  cursor.style.transform = `translate(${frame.cursor.x}px, ${frame.cursor.y}px) scale(${1 - 0.18 * frame.cursor.press})`;
  syncAnimations(now);
}

const ISLAND_EASE = "cubic-bezier(0.2, 1.15, 0.3, 1)";

/**
 * Moves the island to another of its states, the larger one growing out of the
 * smaller from the top of the screen, as the real island does over the notch.
 */
async function island(state: string) {
  const current = document.querySelector<HTMLImageElement>("img.notch")!;
  const next = current.cloneNode() as HTMLImageElement;
  next.src = `/__showcase/notch/${state}.png`;
  await next.decode();
  current.after(next);
  const at = (scaleX: number, scaleY: number, opacity: number) => ({
    transform: `translateX(-50%) scale(${scaleX}, ${scaleY})`,
    opacity,
  });
  const grows = next.naturalHeight > current.naturalHeight;
  const ratio =
    Math.min(next.naturalHeight, current.naturalHeight) /
    Math.max(next.naturalHeight, current.naturalHeight);
  for (const image of [current, next]) image.style.transformOrigin = "50% 0";
  // The old state is gone before the new one shows, so their words never sit on top of each other.
  const leaving = current.animate(
    grows
      ? [{ opacity: 1 }, { opacity: 0 }]
      : [at(1, 1, 1), at(0.62, ratio, 0)],
    {
      duration: grows ? 110 : 220,
      easing: "cubic-bezier(0.4, 0, 1, 1)",
      fill: "forwards",
    },
  );
  const moving = next.animate(
    grows
      ? [at(0.62, ratio, 0), at(1, 1, 1)]
      : [{ opacity: 0 }, { opacity: 1 }],
    {
      duration: grows ? 460 : 200,
      delay: grows ? 100 : 210,
      easing: grows ? ISLAND_EASE : "ease-out",
      fill: "backwards",
    },
  );
  await leaving.finished;
  await moving.finished;
  current.remove();
}

/** The closing card: the window settles away and the mark and the site's address come up on the wallpaper. */
function endCard(): Promise<unknown> {
  const card = document.createElement("div");
  card.className = "video-end-card";
  card.innerHTML = `<img src="/brand/mark/mark-white.svg" alt="" /><span>sikemux.com</span>`;
  document.querySelector(".stage")!.append(card);
  const screen = [...camera.querySelectorAll<HTMLElement>("iframe, img.notch")];
  for (const element of screen)
    element.animate(
      [
        { opacity: 1, filter: "blur(0px)" },
        { opacity: 0, filter: "blur(12px)" },
      ],
      {
        duration: 700,
        easing: "cubic-bezier(0.4, 0, 0.2, 1)",
        fill: "forwards",
      },
    );
  return card.animate(
    [
      { opacity: 0, transform: "translate(-50%, -50%) scale(0.96)" },
      { opacity: 1, transform: "translate(-50%, -50%) scale(1)" },
    ],
    {
      duration: 700,
      delay: 350,
      easing: "cubic-bezier(0.2, 0.8, 0.2, 1)",
      fill: "both",
    },
  ).finished;
}

Object.assign(window, {
  video: {
    endCard,
    rect,
    show,
    island,
    ready: () =>
      app.contentWindow &&
      "showcase" in app.contentWindow &&
      !!app.contentDocument?.querySelector(".shell"),
  },
});
