import { openWindow } from "../scenes.mjs";

export const FPS = 60;
const STEP = 1000 / FPS;
const WIDTH = 1920;
const HEIGHT = 1080;

const clamp = (value, low, high) => Math.min(high, Math.max(low, value));
const lerp = (from, to, at) => from + (to - from) * at;
/** How a hand moves between two points: slow away, fast across, slow onto the target. */
const handEase = (p) => p * p * p * (10 + p * (6 * p - 15));
const cameraEase = (p) => (p < 0.5 ? 4 * p * p * p : 1 - (-2 * p + 2) ** 3 / 2);
const outEase = (p) => 1 - (1 - p) ** 3;

function seeded(seed) {
  let state = seed;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Plays a scene's beats on virtual time, one frame at a time: each frame moves
 * the cursor and the camera, advances every clock in the page by exactly one
 * sixtieth of a second, and hands the picture to `capture`.
 */
export class Director {
  constructor({ page, app, start, capture }) {
    this.page = page;
    this.appFrame = app;
    this.now = start;
    this.capture = capture;
    this.recording = false;
    this.frames = 0;
    this.recorded = 0;
    this.tweens = new Set();
    this.background = new Set();
    this.cursor = { x: 1500, y: 900, press: 0, opacity: 1, shape: null };
    this.view = { x: 0, y: 0, width: WIDTH };
    this.blur = 0;
    this.mouse = null;
    this.random = seeded(7);
  }

  get seconds() {
    return this.recorded / FPS;
  }

  tween(duration, apply) {
    const tween = {
      start: this.frames * STEP,
      duration: Math.max(STEP, duration),
      apply,
    };
    this.tweens.add(tween);
    return tween;
  }

  async until(done, { limit = 20_000, label = "condition" } = {}) {
    const end = this.frames * STEP + limit;
    while (!(await done())) {
      if (this.frames * STEP > end)
        throw new Error(`timed out waiting for ${label}`);
      await this.frame();
    }
  }

  async frame() {
    this.frames += 1;
    this.now += STEP;
    const at = this.frames * STEP;
    for (const task of [...this.background]) await task(at);
    for (const tween of [...this.tweens]) {
      const progress = clamp((at - tween.start) / tween.duration, 0, 1);
      tween.apply(progress);
      if (progress >= 1) this.tweens.delete(tween);
    }
    const scale = WIDTH / this.view.width;
    const mouse = {
      x: (this.cursor.x - this.view.x) * scale,
      y: (this.cursor.y - this.view.y) * scale,
    };
    if (
      this.cursor.opacity > 0 &&
      (!this.mouse ||
        Math.abs(mouse.x - this.mouse.x) > 0.25 ||
        Math.abs(mouse.y - this.mouse.y) > 0.25)
    ) {
      this.mouse = mouse;
      await this.page.mouse.move(mouse.x, mouse.y);
    }
    await Promise.all(
      this.page.frames().map((frame) =>
        frame
          .evaluate((now) => {
            const clock = globalThis.__pwClock?.controller;
            return clock?.runFor(Math.max(0, now - clock.now()));
          }, this.now)
          .catch(() => {}),
      ),
    );
    await this.page.evaluate(
      ([frame, now]) => window.video.show(frame, now),
      [
        {
          camera: {
            x: this.view.x + this.view.width / 2,
            y: this.view.y + (this.view.width * HEIGHT) / WIDTH / 2,
            scale,
            blur: this.blur,
          },
          cursor: this.cursor,
        },
        this.now,
      ],
    );
    if (!this.recording) return;
    this.recorded += 1;
    await this.capture();
  }

  /** Lets the page settle for `ms` without filming it, before the scene starts. */
  async settle(ms = 600) {
    const recording = this.recording;
    this.recording = false;
    await this.wait(ms);
    this.recording = recording;
  }

  async wait(ms) {
    const frames = Math.round(ms / STEP);
    for (let index = 0; index < frames; index++) await this.frame();
  }

  /** Waits for every camera, cursor and blur move still running. */
  async still() {
    while (this.tweens.size) await this.frame();
  }

  async rect(target) {
    const found = await this.page.evaluate(
      (target) => window.video.rect(target),
      target,
    );
    if (!found)
      throw new Error(`nothing on screen matches ${JSON.stringify(target)}`);
    return found;
  }

  /** Plays on until `target` is on screen. */
  async appear(target, { limit = 15_000 } = {}) {
    await this.until(
      async () =>
        (await this.page.evaluate(
          (target) => window.video.rect(target),
          target,
        )) !== null,
      { limit, label: JSON.stringify(target) },
    );
  }

  async point(target, at = { x: 0.5, y: 0.5 }) {
    if (typeof target === "object" && "x" in target && !("width" in target))
      return target;
    const box = await this.rect(target);
    return { x: box.x + box.width * at.x, y: box.y + box.height * at.y };
  }

  app(action, arg) {
    return this.appFrame.evaluate(action, arg);
  }

  /** Shows a project's tab, the way the person would pick it. */
  open(sessionId, windowId) {
    return openWindow(this.appFrame, sessionId, windowId);
  }

  /** Puts the cursor somewhere at once, as if it had been resting there. */
  async place(target, at) {
    const point = await this.point(target, at);
    this.cursor.x = point.x;
    this.cursor.y = point.y;
  }

  /** Glides the cursor onto a target along a slight arc, taking as long as a hand would. */
  async move(target, { at, duration, wait = true } = {}) {
    const to = await this.point(target, at);
    const from = { x: this.cursor.x, y: this.cursor.y };
    const distance = Math.hypot(to.x - from.x, to.y - from.y);
    if (distance < 1) return;
    const time = duration ?? 280 + 210 * Math.log2(1 + distance / 70);
    const bend = (this.random() < 0.5 ? -1 : 1) * Math.min(60, distance * 0.08);
    const control = {
      x: (from.x + to.x) / 2 + ((to.y - from.y) / distance) * bend,
      y: (from.y + to.y) / 2 - ((to.x - from.x) / distance) * bend,
    };
    const tween = this.tween(time, (progress) => {
      const p = handEase(progress);
      this.cursor.x =
        (1 - p) ** 2 * from.x + 2 * (1 - p) * p * control.x + p * p * to.x;
      this.cursor.y =
        (1 - p) ** 2 * from.y + 2 * (1 - p) * p * control.y + p * p * to.y;
    });
    if (wait) await this.until(() => !this.tweens.has(tween));
  }

  async click(target, options = {}) {
    if (target !== undefined) {
      await this.move(target, options);
      await this.wait(options.pause ?? 90);
    }
    await this.pressCursor(1, 70);
    await this.page.mouse.down();
    await this.wait(50);
    await this.page.mouse.up();
    await this.pressCursor(0, 130);
    await this.wait(options.after ?? 160);
  }

  async pressCursor(to, duration) {
    const from = this.cursor.press;
    const tween = this.tween(duration, (p) => {
      this.cursor.press = lerp(from, to, outEase(p));
    });
    await this.until(() => !this.tweens.has(tween));
  }

  /** Types at a person's pace, a little uneven, the same every render. */
  async type(text, { perSecond = 15 } = {}) {
    for (const character of text) {
      await this.page.keyboard.type(character);
      const gap = (1000 / perSecond) * (0.55 + this.random() * 0.9);
      await this.wait(character === " " ? gap * 1.3 : gap);
    }
  }

  async key(name, { after = 200 } = {}) {
    await this.page.keyboard.press(name);
    await this.wait(after);
  }

  /** Moves the camera so `target` fills the screen, or by `scale` around it, never past the screen's edges. */
  async zoom(
    target,
    {
      scale,
      pad = 48,
      at = { x: 0.5, y: 0.5 },
      duration = 1100,
      wait = true,
    } = {},
  ) {
    let width = WIDTH;
    let center = { x: WIDTH / 2, y: HEIGHT / 2 };
    if (target !== null) {
      const box = await this.rect(target);
      const fit = Math.min(
        WIDTH / (box.width + pad * 2),
        HEIGHT / (box.height + pad * 2),
      );
      width = WIDTH / clamp(scale ?? fit, 1, 3);
      center = { x: box.x + box.width * at.x, y: box.y + box.height * at.y };
    }
    const height = (width * HEIGHT) / WIDTH;
    const end = {
      x: clamp(center.x - width / 2, 0, WIDTH - width),
      y: clamp(center.y - height / 2, 0, HEIGHT - height),
      width,
    };
    const start = { ...this.view };
    const tween = this.tween(duration, (progress) => {
      const p = cameraEase(progress);
      this.view.width = start.width * (end.width / start.width) ** p;
      // A zoom keeps one point of the screen still, as a lens would, instead of sliding across it.
      const along =
        Math.abs(end.width - start.width) > 1
          ? (start.width - this.view.width) / (start.width - end.width)
          : p;
      this.view.x = lerp(start.x, end.x, along);
      this.view.y = lerp(start.y, end.y, along);
    });
    if (wait) await this.until(() => !this.tweens.has(tween));
  }

  zoomOut(options = {}) {
    return this.zoom(null, options);
  }

  /** Blurs the picture out, changes it, and brings it back, the way a cut between two moments feels. */
  async cut(change, { duration = 560, strength = 18 } = {}) {
    const half = duration / 2;
    const into = this.tween(half, (p) => {
      this.blur = strength * p * p;
    });
    await this.until(() => !this.tweens.has(into));
    await change();
    await this.wait(STEP * 2);
    const out = this.tween(half, (p) => {
      this.blur = strength * (1 - p) ** 2;
    });
    await this.until(() => !this.tweens.has(out));
  }

  /**
   * Plays the live turns the demo agents have queued, or only `agent`'s, step by
   * step, holding each as long as it took. Without `wait` they play on behind the beats that follow.
   */
  async live({
    agent,
    pace = 1,
    shortest = 250,
    steps = Infinity,
    wait = true,
  } = {}) {
    let due = this.frames * STEP;
    let taken = 0;
    const task = async (at) => {
      if (at < due) return;
      const hold =
        taken++ < steps
          ? await this.app((agent) => showcase.backend.stepLive(agent), agent)
          : -1;
      if (hold < 0) this.background.delete(task);
      else due = at + Math.max(shortest, hold * pace);
    };
    this.background.add(task);
    if (wait)
      await this.until(() => !this.background.has(task), {
        limit: 120_000,
        label: "the live turn",
      });
  }

  /** Scrolls `selector`, or the first thing inside it that scrolls, smoothly by `by` pixels. */
  async scroll(selector, { by, duration = 1600, ease = cameraEase } = {}) {
    const from = await this.app((selector) => {
      const root = [...document.querySelectorAll(selector)].find(
        (element) => element.getClientRects().length,
      );
      const scrolls = (element) =>
        element.scrollHeight > element.clientHeight + 1 &&
        /auto|scroll/.test(getComputedStyle(element).overflowY);
      const scroller =
        root &&
        (scrolls(root) ? root : [...root.querySelectorAll("*")].find(scrolls));
      if (!scroller) return null;
      window.__videoScroller = scroller;
      return scroller.scrollTop;
    }, selector);
    if (from === null)
      throw new Error(`nothing in the app under ${selector} scrolls`);
    let top = from;
    const tween = this.tween(duration, (p) => {
      top = from + by * ease(p);
    });
    while (this.tweens.has(tween)) {
      await this.frame();
      await this.app((top) => {
        window.__videoScroller.scrollTop = top;
      }, top);
    }
  }

  /** A two-finger swipe over `target`: the hand lands, pushes `pixels` sideways and lifts. */
  async swipe(target, { pixels = 220, duration = 380 } = {}) {
    await this.move(target);
    await this.app(() => showcase.backend.emit("wheel-touch", true));
    const frames = Math.round(duration / STEP);
    let pushed = 0;
    for (let index = 1; index <= frames; index++) {
      const next = pixels * outEase(index / frames);
      await this.page.mouse.wheel(next - pushed, 0);
      pushed = next;
      await this.frame();
    }
    await this.app(() => showcase.backend.emit("wheel-touch", false));
  }

  /** Moves the island over the notch to one of its states, as drawn by its snapshot tool. */
  async island(state) {
    const settled = this.page.evaluate(
      (state) => window.video.island(state),
      state,
    );
    let done = false;
    settled.then(() => (done = true));
    await this.until(() => done, { label: `the island to show ${state}` });
  }

  async hideCursor(duration = 240) {
    await this.fadeCursor(0, duration);
  }

  async showCursor(duration = 240) {
    await this.fadeCursor(1, duration);
  }

  async fadeCursor(to, duration) {
    const from = this.cursor.opacity;
    const tween = this.tween(duration, (p) => {
      this.cursor.opacity = lerp(from, to, p);
    });
    await this.until(() => !this.tweens.has(tween));
  }
}
