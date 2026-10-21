import { ChatAttachment, MobileError, type ChatMark, type ConnectionLike } from '@sikemux/native';
import { randomUUID } from 'expo-crypto';
import { File } from 'expo-file-system';

import { answerText } from '@mac/chat/answerText';
import { applyChanges, emptyChat, fromView, withPage, type ChatRows } from '@mac/chat/rows';
import type { ChatChange, ChatMessage, ChatPage, ChatView, PermissionAsk, QueuedMessage, ToolDetail } from '@mac/chat/types';

import { problem as problemOf } from '@/core/problem';
import type { ChatInfo } from '@/core/protocol';
import type { ChatDelivery } from '@/devices/hub';
import { Folds } from './folds';
import type { ChatStore, Kept } from './kept';
import { fitting, pathsOf, uploadEach, type Attachment } from './uploads';

export type Attached = 'attaching' | 'live' | 'missing';

/** A message this phone wrote that the host has not taken yet. */
export type Unsent = { state: 'sending' | 'failed'; problem?: string };

/** A message waiting in the host's queue behind the running turn. */
export type Held = QueuedMessage;

export type ChatSnapshot = {
  /** The chat as the host folded it, with what this phone sent and the host does not show yet after it. */
  view: ChatRows;
  /** The permission requests still waiting on the host. */
  permissions: PermissionAsk[];
  attached: Attached;
  /** Why the chat could not be opened, in the host's words. */
  problem: string | null;
  /** Something this phone asked for that did not happen, such as a stop or a setting. */
  notice: string | null;
  /** By message id. */
  unsent: ReadonlyMap<string, Unsent>;
  /** Picked for the next message. */
  attachments: readonly Attachment[];
  /** What this phone sent, by the path the host keeps it at. */
  sentFiles: ReadonlyMap<string, Attachment>;
  /** Permission requests whose answer is on its way. */
  answering: ReadonlySet<string>;
  /** Background tasks this phone asked the host to stop. */
  stopping: ReadonlySet<string>;
  /** How the chat asks before acting, as this phone last set it or the host last said; null before either. */
  permissionMode: string | null;
  changingMode: boolean;
  hasEarlier: boolean;
  earlier: 'idle' | 'loading' | 'failed';
  /** When the running turn began. */
  turnSince: number;
};

type Options = {
  /** Hands over this host's chat changes as they arrive. */
  listen: (take: (deliveries: ChatDelivery[]) => void) => () => void;
  /** Runs once before the next frame is drawn; returns how to call that off. */
  frame?: (run: () => void) => () => void;
  now?: () => number;
  /** The contents of a file on this phone. */
  read?: (uri: string) => Promise<ArrayBuffer>;
  /** Where the chat is kept between launches. */
  store?: ChatStore;
  /** Told how long each step of taking the chat up took, to measure it. */
  timed?: (took: OpenTimes) => void;
};

/** In milliseconds, from when the phone began taking the chat up. */
export type OpenTimes = {
  /** Reading the chat as this phone last kept it, or null when it had not. */
  restore: number | null;
  woke: boolean;
  answered: number;
  answer: 'live' | 'resumed' | 'missing';
  bytes: number;
  applied: number;
  drawn: number;
};

/** `sent` is taken by the host and not shown by it yet. */
type Outgoing = { messageId: string; text: string; paths: string[]; at: number; state: Unsent['state'] | 'sent'; problem?: string };

/** Turns of earlier rows asked for at a time. */
const PAGE_TURNS = 20;

/** Finished calls whose change and output are kept once read, so a row opened again shows them at once. */
const KEPT_DETAILS = 40;

/**
 * A permission request that just arrived is shown before the host's list of waiting requests,
 * which comes on its own schedule, has caught up with it.
 */
const PERMISSION_GRACE_MS = 2000;

function readFile(uri: string): Promise<ArrayBuffer> {
  return new File(uri).arrayBuffer();
}

function animationFrame(run: () => void): () => void {
  const frame = requestAnimationFrame(run);
  return () => cancelAnimationFrame(frame);
}

function ownRow(outgoing: Outgoing): ChatMessage {
  return {
    id: outgoing.messageId,
    role: 'user',
    promptId: outgoing.messageId,
    parts: outgoing.text.trim() ? [{ kind: 'text', id: `${outgoing.messageId}/text`, text: outgoing.text }] : [],
    attachments: outgoing.paths,
    sentAt: outgoing.at,
  };
}

const sameItems = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((item, index) => item === b[index]);

/**
 * One chat on one host, kept outside React so it outlives the screen: the chat as the host folds it,
 * where its changes got to, what this phone sent and what it still means to send.
 */
export class ChatSession {
  readonly folds = new Folds();
  private listeners = new Set<() => void>();
  private chat: ChatRows = emptyChat;
  private attached: Attached = 'attaching';
  private problem: string | null = null;
  private notice: string | null = null;
  private outbox: Outgoing[] = [];
  private attachments: Attachment[] = [];
  private sentFiles: ReadonlyMap<string, Attachment> = new Map();
  private answering: ReadonlySet<string> = new Set();
  private stopping: ReadonlySet<string> = new Set();
  /** The mode this phone set, until the host says another. */
  private mode: string | null = null;
  private changingMode = false;
  private draft = '';
  /** When this phone saw the running turn begin, for a turn the host has no time for. */
  private turnSince: number;

  /** The last change the chat holds; a reconnect asks only for what comes after it. */
  private mark: ChatMark | undefined;
  /** The last change taken, applied or not yet. */
  private taken: bigint | undefined;
  private pending: ChatChange[] = [];
  private cancelFrame: (() => void) | undefined;
  private earlier: ChatSnapshot['earlier'] = 'idle';
  private details = new Map<string, Promise<ToolDetail>>();

  /** What the host lists as waiting, or null before it has said. */
  private onHost: ReadonlySet<string> | null = null;
  private arrived = new Map<string, number>();
  private hostUp: boolean | undefined;

  private connection: ConnectionLike | undefined;
  private holders = 0;
  /** Counts attaches, so an answer that comes back after the chat was taken up again is dropped. */
  private run = 0;
  private stopRun: (() => void) | undefined;
  private snap: ChatSnapshot;
  private keptChat: ChatRows | undefined;
  private restoreTook: number | null = null;

  constructor(
    readonly agentId: string,
    private readonly options: Options,
  ) {
    this.turnSince = this.now();
    const reading = performance.now();
    const kept = options.store?.load();
    if (kept) {
      this.restore(kept);
      this.restoreTook = performance.now() - reading;
    }
    this.snap = this.build();
  }

  /** Writes the chat down as drawn, unless it has not changed since it last was. */
  keep = () => {
    const store = this.options.store;
    const mark = this.mark;
    if (!store || !mark || this.chat === this.keptChat) return;
    this.keptChat = this.chat;
    store.save({ chat: this.chat, mark: { feed: mark.feed, seq: String(mark.seq) } });
  };

  /** Draws the chat as this phone last kept it; the first attach asks only for what came after. */
  private restore(kept: Kept) {
    this.chat = kept.chat;
    this.keptChat = kept.chat;
    this.mark = { feed: kept.mark.feed, seq: BigInt(kept.mark.seq) };
    this.taken = this.mark.seq;
    this.attached = 'live';
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  snapshot = () => this.snap;

  draftText = () => this.draft;

  setDraft = (text: string) => {
    if (text === this.draft) return;
    this.draft = text;
    this.notify();
  };

  /** Keeps the chat attached over this connection until the returned function is called. */
  hold = (connection: ConnectionLike | undefined) => {
    if (!connection) return () => {};
    this.holders += 1;
    if (this.connection !== connection) {
      this.stop();
      this.connection = connection;
      this.start();
    }
    return () => {
      this.holders -= 1;
      if (this.holders > 0 || this.connection !== connection) return;
      this.stop();
      this.connection = undefined;
    };
  };

  /**
   * What the host's own view says of this chat. A chat that comes back after it slept, stopped or
   * restarted is taken up again, and only the requests the host still waits on are shown.
   */
  hostSaw = (info: ChatInfo | undefined, waiting: string[] | undefined) => {
    const up = info !== undefined && info.state === 'ready' && !info.asleep;
    const cameBack = this.hostUp === false && up;
    this.hostUp = up;
    const next = waiting ? new Set(waiting) : null;
    const same = next && this.onHost && next.size === this.onHost.size && [...next].every((id) => this.onHost?.has(id));
    if (!same && (next || this.onHost)) {
      this.onHost = next;
      this.emit();
    }
    if (cameBack && this.connection && this.attached !== 'attaching') this.restart();
  };

  /** Adds what was picked for the next message, leaving out what the host would refuse. */
  attach = (picked: Attachment[]) => {
    const { fits, notice } = fitting(this.attachments.length, picked);
    this.attachments = [...this.attachments, ...fits];
    this.notice = notice;
    this.emit();
  };

  removeAttachment = (id: string) => {
    if (this.uploading()) return;
    this.attachments = this.attachments.filter((attachment) => attachment.id !== id);
    this.emit();
  };

  /**
   * Sends the message once every file picked for it is on the host. Answers whether it went out
   * or waits behind the turn; when a file did not get there, it stays in the composer to try again.
   */
  send = async (text: string): Promise<boolean> => {
    this.notice = null;
    const attachments = this.attachments.length ? await this.upload() : [];
    if (!attachments) return false;
    const paths = pathsOf(attachments);
    if (this.busy()) {
      void this.ask((connection) => this.prompt(connection, randomUUID(), text, paths), 'Could not send the message');
      return true;
    }
    const entry: Outgoing = { messageId: randomUUID(), text, paths, at: this.now(), state: 'sending' };
    this.outbox = [...this.outbox, entry];
    this.emit();
    this.push(entry);
    return true;
  };

  /**
   * Answers a question with `text` and the files picked for it, named in the answer once they are all on the
   * host. The files stay in the composer when the answer did not go out.
   */
  answerWith = async (text: string, answer: (text: string) => Promise<boolean>): Promise<boolean> => {
    this.notice = null;
    const attachments = this.attachments.length ? await this.upload() : [];
    if (!attachments) return false;
    if (await answer(answerText(text, pathsOf(attachments)))) return true;
    this.attachments = [...attachments, ...this.attachments];
    this.emit();
    return false;
  };

  retrySend = (messageId: string) => {
    const entry = this.outbox.find((outgoing) => outgoing.messageId === messageId);
    if (!entry || entry.state !== 'failed') return;
    this.update(messageId, { state: 'sending', problem: undefined });
    this.push(entry);
  };

  /** Puts waiting messages into the running turn now, as one, instead of after it. */
  steer = (ids: readonly string[]) => {
    void this.ask((connection) => connection.call('steerQueued', JSON.stringify({ agentId: this.agentId, ids })), 'Could not steer');
  };

  dropQueued = (id: string) => {
    void this.ask(
      (connection) => connection.call('dropQueued', JSON.stringify({ agentId: this.agentId, ids: [id] })),
      'Could not take it back',
    );
  };

  /** Stops the running turn. Messages waiting behind it go back to the composer rather than out. */
  cancel = () => {
    const queued = this.chat.queued;
    if (queued.length) {
      const text = queued.map((held) => held.text).filter(Boolean);
      const paths = [...new Set(queued.flatMap((held) => held.paths))];
      this.draft = [...text, this.draft].filter(Boolean).join('\n\n');
      const sent = paths.flatMap((path) => {
        const attachment = this.sentFiles.get(path);
        return attachment ? [{ ...attachment, path }] : [];
      });
      this.attachments = [...sent, ...this.attachments];
    }
    this.notice = null;
    this.emit();
    this.ask((connection) => connection.cancel(this.agentId), 'Could not stop the turn');
  };

  answer = (requestId: string, optionId: string | null) => {
    if (this.answering.has(requestId)) return;
    this.answering = new Set([...this.answering, requestId]);
    this.notice = null;
    this.emit();
    this.ask(
      (connection) => connection.answerPermission(this.agentId, requestId, optionId ?? undefined),
      'The answer did not reach the host',
      () => {
        this.chat = { ...this.chat, permissions: this.chat.permissions.filter((request) => request.requestId !== requestId) };
      },
    ).finally(() => {
      this.answering = new Set([...this.answering].filter((id) => id !== requestId));
      this.emit();
    });
  };

  stopTask = (taskId: string) => {
    if (this.stopping.has(taskId)) return;
    this.stopping = new Set([...this.stopping, taskId]);
    this.notice = null;
    this.emit();
    this.ask((connection) => connection.stopTask(this.agentId, taskId), 'Could not stop the task').finally(() => {
      this.stopping = new Set([...this.stopping].filter((id) => id !== taskId));
      this.emit();
    });
  };

  /** `bypass` runs without asking; `workspace-write` asks first. */
  setPermissionMode = (mode: string) => {
    if (this.changingMode) return;
    this.changingMode = true;
    this.notice = null;
    this.emit();
    this.ask(
      (connection) => connection.setPermissionMode(this.agentId, mode),
      'Could not change permissions',
      () => {
        this.mode = mode;
      },
    ).finally(() => {
      this.changingMode = false;
      this.emit();
    });
  };

  /** Ends the chat's agent on the host and closes it in every app. Answers whether it did. */
  stopChat = async (): Promise<boolean> => {
    let stopped = false;
    await this.ask(
      (connection) => connection.call('closeChat', JSON.stringify({ agentId: this.agentId })),
      'Could not stop the chat',
      () => {
        stopped = true;
      },
    );
    return stopped;
  };

  setConfig = (configId: string, value: string) => {
    this.notice = null;
    this.emit();
    this.ask((connection) => connection.setChatConfig(this.agentId, configId, value), 'Could not change the setting');
  };

  /** Says what went wrong where the person is looking. */
  report = (text: string) => {
    this.notice = text;
    this.emit();
  };

  dismissNotice = () => {
    this.notice = null;
    this.emit();
  };

  /** Fetches the rows before the first one shown, unless that is already under way. */
  loadEarlier = () => {
    const connection = this.connection;
    const before = this.chat.olderBefore;
    const feed = this.mark?.feed;
    if (before === null || feed === undefined || !connection || this.earlier === 'loading') return;
    this.earlier = 'loading';
    this.emit();
    const run = this.run;
    connection
      .chatHistory(this.agentId, feed, BigInt(before), PAGE_TURNS)
      .then((json) => {
        if (run !== this.run) return;
        this.chat = withPage(this.chat, JSON.parse(json) as ChatPage);
        this.earlier = 'idle';
        this.emit();
      })
      .catch((error: unknown) => {
        if (run !== this.run) return;
        if (MobileError.Refused.instanceOf(error)) {
          this.chat = { ...this.chat, olderBefore: null };
          this.earlier = 'idle';
        } else {
          this.earlier = 'failed';
        }
        this.emit();
      });
  };

  /** A call's whole change and what it printed, which rows carry only counts of. `done` calls are read once. */
  toolDetail = (toolCallId: string, done: boolean): Promise<ToolDetail> => {
    const kept = this.details.get(toolCallId);
    if (kept) return kept;
    const connection = this.connection;
    const reading = Promise.resolve()
      .then(() => {
        if (!connection?.isOpen()) throw new Error('the host is not connected');
        return connection.call('chatToolDetail', JSON.stringify({ agentId: this.agentId, toolCallId }));
      })
      .then((json) => JSON.parse(json) as ToolDetail);
    if (!done) return reading;
    this.details.set(toolCallId, reading);
    reading.catch(() => this.details.delete(toolCallId));
    if (this.details.size > KEPT_DETAILS) this.details.delete(this.details.keys().next().value as string);
    return reading;
  };

  /** Opens the chat again from the start, after it could not be opened. */
  retry = () => {
    this.mark = undefined;
    this.taken = undefined;
    if (this.connection) this.restart();
  };

  private now() {
    return (this.options.now ?? Date.now)();
  }

  private notify() {
    this.listeners.forEach((listener) => listener());
  }

  private emit() {
    this.snap = this.build();
    this.notify();
  }

  private build(): ChatSnapshot {
    const now = this.now();
    const onHost = this.onHost;
    const all = this.chat.permissions;
    const permissions = onHost
      ? all.filter(
          (request) => onHost.has(request.requestId) || now - (this.arrived.get(request.requestId) ?? -Infinity) < PERMISSION_GRACE_MS,
        )
      : all;
    return {
      view: this.view(),
      permissions: permissions.length === all.length ? all : permissions,
      attached: this.attached,
      problem: this.problem,
      notice: this.notice,
      unsent: this.unsentView(),
      attachments: this.attachments,
      sentFiles: this.sentFiles,
      answering: this.answering,
      stopping: this.stopping,
      permissionMode: this.mode ?? (this.chat.permissionMode || null),
      changingMode: this.changingMode,
      hasEarlier: this.chat.olderBefore !== null,
      earlier: this.earlier,
      turnSince: this.since(),
    };
  }

  private viewOf: { chat: ChatRows; outbox: Outgoing[]; view: ChatRows } | undefined;

  private view(): ChatRows {
    if (this.viewOf?.chat === this.chat && this.viewOf.outbox === this.outbox) return this.viewOf.view;
    const waiting = this.outbox.filter((outgoing) => outgoing.state !== 'failed');
    const view = this.outbox.length
      ? {
          ...this.chat,
          messages: [...this.chat.messages, ...this.outbox.map(ownRow)],
          running: this.chat.running || waiting.length > 0,
        }
      : this.chat;
    this.viewOf = { chat: this.chat, outbox: this.outbox, view };
    return view;
  }

  private unsentFrom: Outgoing[] | undefined;
  private unsentMap: ReadonlyMap<string, Unsent> = new Map();

  private unsentView(): ReadonlyMap<string, Unsent> {
    if (this.unsentFrom !== this.outbox) {
      this.unsentFrom = this.outbox;
      this.unsentMap = new Map(
        this.outbox.flatMap(({ messageId, state, problem }) => (state === 'sent' ? [] : [[messageId, { state, problem }]])),
      );
    }
    return this.unsentMap;
  }

  /** When the running turn began: as the host timed it, or else as this phone saw it. */
  private since(): number {
    const { turnOpening, messages } = this.view();
    if (turnOpening?.at) return turnOpening.at;
    for (let at = messages.length - 1; at >= 0; at -= 1) {
      const row = messages[at];
      if (row.role === 'user') return row.sentAt ?? this.turnSince;
      if (row.opensTurn) return row.opensTurn.at ?? this.turnSince;
    }
    return this.turnSince;
  }

  private uploading() {
    return this.attachments.some((attachment) => attachment.upload === 'sending');
  }

  private patch(id: string, change: Partial<Attachment>) {
    this.attachments = this.attachments.map((attachment) => (attachment.id === id ? { ...attachment, ...change } : attachment));
    this.emit();
  }

  /** Puts the picked files on the host and takes them out of the composer, or answers null when one did not get there. */
  private async upload(): Promise<Attachment[] | null> {
    if (this.uploading()) return null;
    const connection = this.connection;
    const sent = await uploadEach(
      this.attachments,
      async (attachment) => {
        if (!connection?.isOpen()) throw new Error('the host is not connected');
        const bytes = await (this.options.read ?? readFile)(attachment.uri);
        const path = await connection.attachFile(this.agentId, attachment.name, attachment.mime, bytes);
        this.sentFiles = new Map([...this.sentFiles, [path, attachment]]);
        return path;
      },
      (id, change) => this.patch(id, change),
      problemOf,
    );
    if (sent) this.attachments = [];
    return sent;
  }

  private busy() {
    return this.chat.running || this.outbox.some((outgoing) => outgoing.state !== 'failed');
  }

  /** Lays the host's changes over the chat, and lets go of what this phone sent once the host shows it. */
  private apply(change: (chat: ChatRows) => ChatRows, live = false) {
    const before = this.chat;
    this.chat = change(before);
    if (!before.running && this.chat.running) this.turnSince = this.now();
    if (this.chat.permissionMode !== before.permissionMode) this.mode = null;
    if (live && this.chat.permissions !== before.permissions) {
      for (const request of this.chat.permissions) {
        if (before.permissions.some((held) => held.requestId === request.requestId)) continue;
        this.arrived.set(request.requestId, this.now());
        setTimeout(() => this.emit(), PERMISSION_GRACE_MS);
      }
    }
    this.settleOutbox(before.running && !this.chat.running);
  }

  /* The host shows a message as the row that carries its id; one that met a running turn waits in
     the host's queue instead, under an id of the host's own. Once a turn has ended, nothing the host took is still to come. */
  private settleOutbox(turnEnded = false) {
    if (!this.outbox.some((outgoing) => outgoing.state === 'sent')) return;
    if (turnEnded) {
      this.outbox = this.outbox.filter((outgoing) => outgoing.state !== 'sent');
      return;
    }
    const shown = new Set(this.chat.messages.flatMap((row) => (row.promptId ? [row.promptId] : [])));
    const queued = this.chat.queued;
    const kept = this.outbox.filter(
      (outgoing) =>
        outgoing.state !== 'sent' ||
        (!shown.has(outgoing.messageId) && !queued.some((held) => held.text === outgoing.text && sameItems(held.paths, outgoing.paths))),
    );
    if (kept.length !== this.outbox.length) this.outbox = kept;
  }

  private take(deliveries: ChatDelivery[]) {
    for (const delivery of deliveries) {
      if (this.taken !== undefined && delivery.seq <= this.taken) continue;
      this.pending.push(...(JSON.parse(delivery.changesJson) as ChatChange[]));
      this.taken = delivery.seq;
    }
    // A stream of tokens is drawn once a frame, not once a change.
    this.cancelFrame ??= (this.options.frame ?? animationFrame)(this.flush);
  }

  private flush = () => {
    this.applyPending();
  };

  /** Applies what was taken, and only then moves the mark past it. */
  private applyPending() {
    this.cancelFrame?.();
    this.cancelFrame = undefined;
    const advanced = this.mark && this.taken !== undefined && this.taken !== this.mark.seq;
    if (!this.pending.length && !advanced) return;
    const batch = this.pending;
    this.pending = [];
    if (batch.length) this.apply((chat) => applyChanges(chat, batch), true);
    if (this.mark && this.taken !== undefined) this.mark = { ...this.mark, seq: this.taken };
    this.emit();
  }

  private restart() {
    this.stop();
    this.start();
  }

  private stop() {
    this.run += 1;
    this.stopRun?.();
    this.stopRun = undefined;
    if (this.earlier === 'loading') this.earlier = 'idle';
    this.keep();
  }

  private start() {
    const connection = this.connection;
    if (!connection) return;
    const run = (this.run += 1);
    const agentId = this.agentId;
    if (!this.mark || this.attached === 'missing') this.attached = 'attaching';
    this.problem = null;
    this.emit();
    let held: ChatDelivery[] | undefined = [];
    const off = this.options.listen((deliveries) => {
      if (run !== this.run) return;
      const mine = deliveries.filter((delivery) => delivery.agentId === agentId);
      if (!mine.length) return;
      // Until the attach answer says where the chat it sends stands, changes wait.
      if (held) held.push(...mine);
      else this.take(mine);
    });
    this.stopRun = () => {
      off();
      // What arrived but was not applied yet goes into the chat now, so the next attach picks up after it.
      this.applyPending();
      if (connection.isOpen()) connection.detachChat(agentId).catch(() => {});
    };

    const began = performance.now();
    let woke = false;
    const wakeAndAttach = () => {
      woke = true;
      return connection.wakeChat(agentId).then(() => connection.attachChat(agentId, this.mark));
    };
    // A chat the host says is up is taken up at once; one asleep, or gone since the host said so, is started first.
    const attaching = this.hostUp
      ? connection
          .attachChat(agentId, this.mark)
          .then((attachment) => (ChatAttachment.Missing.instanceOf(attachment) ? wakeAndAttach() : attachment))
      : wakeAndAttach();
    attaching
      .then((attachment) => {
        if (run !== this.run) return;
        const answered = performance.now() - began;
        const timed = (answer: OpenTimes['answer'], bytes: number) => {
          const report = this.options.timed;
          if (!report) return;
          const applied = performance.now() - began;
          (this.options.frame ?? animationFrame)(() =>
            report({ restore: this.restoreTook, woke, answered, answer, bytes, applied, drawn: performance.now() - began }),
          );
        };
        if (ChatAttachment.Live.instanceOf(attachment)) {
          const view = JSON.parse(attachment.inner.viewJson) as ChatView;
          this.pending = [];
          this.apply(() => fromView(view));
          this.mark = attachment.inner.mark;
          timed('live', attachment.inner.viewJson.length);
        } else if (ChatAttachment.Resumed.instanceOf(attachment)) {
          const changes = JSON.parse(attachment.inner.changesJson) as ChatChange[];
          this.apply((chat) => applyChanges(chat, changes));
          this.mark = attachment.inner.mark;
          timed('resumed', attachment.inner.changesJson.length);
        } else {
          this.attached = 'missing';
          this.emit();
          timed('missing', 0);
          return;
        }
        this.taken = this.mark.seq;
        this.hostUp = true;
        const waiting = held ?? [];
        held = undefined;
        this.attached = 'live';
        this.take(waiting);
        this.emit();
      })
      .catch((error: unknown) => {
        if (run !== this.run) return;
        this.problem = problemOf(error);
        this.attached = 'missing';
        this.emit();
      });
  }

  private prompt(connection: ConnectionLike, messageId: string, text: string, paths: string[]) {
    return connection.prompt(this.agentId, messageId, text, paths);
  }

  private push(entry: Outgoing) {
    const connection = this.connection;
    Promise.resolve()
      .then(() => {
        if (!connection?.isOpen()) throw new Error('the host is not connected');
        return this.prompt(connection, entry.messageId, entry.text, entry.paths);
      })
      .then(() => {
        this.update(entry.messageId, { state: 'sent' });
        this.settleOutbox();
        this.emit();
      })
      .catch((error: unknown) => {
        this.update(entry.messageId, { state: 'failed', problem: problemOf(error) });
      });
  }

  private update(messageId: string, change: Partial<Outgoing>) {
    this.outbox = this.outbox.map((outgoing) => (outgoing.messageId === messageId ? { ...outgoing, ...change } : outgoing));
    this.emit();
  }

  private ask<T>(request: (connection: ConnectionLike) => Promise<T>, what: string, then?: (answer: T) => void): Promise<void> {
    const connection = this.connection;
    return Promise.resolve()
      .then(() => {
        if (!connection?.isOpen()) throw new Error('the host is not connected');
        return request(connection);
      })
      .then((answer) => {
        then?.(answer);
        this.emit();
      })
      .catch((error: unknown) => {
        this.notice = `${what}: ${problemOf(error)}`;
        this.emit();
      });
  }
}
