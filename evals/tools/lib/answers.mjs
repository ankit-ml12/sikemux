import { valueMatches, toolMatches } from './match.mjs';

const GRAY_PNG =
  'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAAA35e4mAAAAP0lEQVR42u3OMQ0AAAgDsM2/aFBBwtEq6NRgtSR6TQFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWHhxwIL2wH5lQAKdAAAAABJRU5ErkJggg==';

const BLANK_PAGE = { url: 'about:blank', title: '', elements: [], text: '' };

export function answerer(fixture, project) {
  const rules = (fixture.answers ?? []).map((rule) => ({ ...rule, used: 0 }));
  const pages = fixture.pages ?? [];
  const me = fixture.me ?? 'Juniper';
  const agents = [
    { name: me, agentId: 'agent-eval-self', cli: 'claude', status: 'working', you: true },
    ...(fixture.agents ?? []).map((agent, index) => ({
      agentId: `agent-eval-${index + 1}`,
      cli: 'claude',
      status: 'idle',
      canMessage: true,
      ...agent,
    })),
  ];
  const state = { tabs: [], active: null, tabCount: 0, executions: 0, cursor: 100 };
  if (fixture.tab) openTab(state, findPage(pages, fixture.tab, fixture.tab));

  return (tool, args) => {
    const rule = rules.find(
      (candidate) =>
        toolMatches(candidate.tool, tool) &&
        (!candidate.args || valueMatches(candidate.args, args)) &&
        (candidate.times === undefined || candidate.used < candidate.times),
    );
    if (rule) {
      rule.used += 1;
      if (rule.page) openTab(state, findPage(pages, rule.page, rule.page), !args.newTab);
      if (rule.error) return { error: rule.error };
      return { value: rule.reply ?? {} };
    }
    return builtIn(tool, args ?? {}, { state, pages, agents, project, fixture });
  };
}

function findPage(pages, url, fallbackUrl) {
  const page = pages.find((candidate) => valueMatches(candidate.url, url) || candidate.url === url);
  return page ? { ...BLANK_PAGE, ...page, url: page.href ?? url } : { ...BLANK_PAGE, url: fallbackUrl, title: fallbackUrl };
}

function openTab(state, page, replace = false) {
  if (replace && state.active) {
    state.active.page = page;
    return state.active;
  }
  const tab = { tabId: `browser-agent-eval-${state.tabCount++}`, page };
  state.tabs.push(tab);
  state.active = tab;
  return tab;
}

function tabOf(state, args) {
  if (args.tabId) return state.tabs.find((tab) => tab.tabId === args.tabId);
  return state.active;
}

function elementsText(page) {
  return page.elements.map((line, index) => (line.startsWith('[') ? line : `[${index}] ${line}`)).join('\n');
}

function where(tab) {
  return { tabId: tab.tabId, url: tab.page.url, title: tab.page.title, loading: false };
}

function pageState(tab, full) {
  return {
    ...where(tab),
    scroll: { y: 0, height: 2400, viewport: 900 },
    viewport: { width: 1280, height: 900 },
    elements: elementsText(tab.page),
    ...(full && tab.page.text ? { text: tab.page.text } : {}),
  };
}

const NO_TAB = 'No desk tab is open for this agent. Open one with browser_navigate.';

function builtIn(tool, args, { state, pages, agents, project, fixture }) {
  const tab = tabOf(state, args);
  const needTab = (answer) => (tab ? { value: answer() } : { error: NO_TAB });
  const outcome = args.report === 'outcome';
  switch (tool) {
    case 'browser_navigate': {
      const page = findPage(pages, args.url ?? '', args.url ?? 'about:blank');
      const opened = openTab(state, page, !args.newTab);
      const met = args.waitFor ? { met: true, waitedMs: 240 } : {};
      return { value: outcome ? { ...met, ...where(opened) } : { ...met, ...pageState(opened, false) } };
    }
    case 'browser_state':
      return needTab(() => pageState(tab, args.fullText));
    case 'browser_find':
      return needTab(() => {
        const wanted = String(args.query ?? args.text ?? '').toLowerCase();
        const lines = elementsText(tab.page)
          .split('\n')
          .filter((line) => line && (!wanted || line.toLowerCase().includes(wanted)));
        return lines.length
          ? { matches: lines.length, elements: lines.join('\n') }
          : { matches: 0, elements: '', note: 'nothing on the page shows those words' };
      });
    case 'browser_click':
      return needTab(() => ({
        ...(args.index !== undefined ? { index: args.index } : {}),
        label: args.text ?? args.selector ?? tab.page.elements[args.index] ?? 'element',
        action: 'clicked',
        ...(outcome ? where(tab) : pageState(tab, false)),
      }));
    case 'browser_type':
      return needTab(() => ({ typed: String(args.text ?? '').length, into: args.selector ?? 'field', submitted: Boolean(args.submit), ...where(tab) }));
    case 'browser_press':
      return needTab(() => ({ pressed: args.key, ...where(tab) }));
    case 'browser_act':
      return needTab(() => ({ steps: (args.steps ?? []).length, done: true, ...where(tab) }));
    case 'browser_scroll':
      return needTab(() => ({ y: args.deltaY ?? 600, height: 2400, viewport: 900, atBottom: false }));
    case 'browser_wait':
      return needTab(() => ({ met: true, waitedMs: 320, ...where(tab) }));
    case 'browser_evaluate':
      return needTab(() => ({ result: null }));
    case 'browser_screenshot':
      return needTab(() => ({ data: GRAY_PNG, mimeType: 'image/png', title: tab.page.title, url: tab.page.url }));
    case 'browser_viewport':
      return needTab(() => ({ viewport: { width: args.width ?? 1280, height: args.height ?? 900, preset: args.preset ?? null }, ...where(tab) }));
    case 'browser_switch_tab': {
      const target = state.tabs.find((candidate) => candidate.tabId === args.tabId);
      if (args.tabId && !target) return { error: `No tab ${args.tabId}` };
      if (target) state.active = target;
      return { value: { activeTabId: state.active?.tabId ?? null, tabs: state.tabs.map((one) => where(one)) } };
    }
    case 'browser_close_tab':
      state.tabs = state.tabs.filter((candidate) => candidate.tabId !== (args.tabId ?? state.active?.tabId));
      state.active = state.tabs.at(-1) ?? null;
      return { value: { activeTabId: state.active?.tabId ?? null, tabs: state.tabs.map((one) => where(one)) } };
    case 'browser_console':
      return needTab(() => ({ recording: true, url: tab.page.url, recorded: 0, matched: 0, messages: [] }));
    case 'browser_network':
      return needTab(() => ({ recording: true, recorded: 0, requests: [] }));
    case 'browser_upload':
      return needTab(() => ({ uploaded: (args.paths ?? []).map((path) => path.split('/').at(-1)), ...where(tab) }));
    case 'browser_dialog':
      return needTab(() => ({ dialog: null }));
    case 'page_preview':
      return { value: { data: GRAY_PNG, mimeType: 'image/png', width: 756, contentHeight: 900, console: { recorded: 0, matched: 0, messages: [] } } };
    case 'page_show':
      return {
        value: {
          page: { id: 'page-eval-1', title: args.title ?? 'Page', height: args.height ?? 900 },
          note: 'The person sees the page in your reply, above the text you write next. Do not describe or restate it; write only what it does not already say.',
        },
      };
    case 'task_start': {
      const executionId = `exec-eval-${++state.executions}`;
      return { value: { executionId, taskId: `sh:${args.label ?? 'task'}`, project, status: 'running', label: args.label ?? args.command, command: args.command, ready: false } };
    }
    case 'task_read':
      return { value: { executionId: args.executionId, status: 'exited', exitCode: 0, output: fixture.taskOutput ?? 'done\n' } };
    case 'task_stop':
    case 'task_restart':
      return { value: { executionId: args.executionId, status: tool === 'task_stop' ? 'stopped' : 'running' } };
    case 'events_wait':
      return {
        value: {
          events: [{ kind: 'task.exited', executionId: args.executionId ?? 'exec-eval-1', exitCode: 0 }],
          cursor: String(++state.cursor),
        },
      };
    case 'agent_list':
      return { value: { project, agents } };
    case 'agent_send': {
      const agent = agents.find((one) => one.name.toLowerCase() === String(args.to).toLowerCase() && !one.you);
      if (!agent) return { error: `No open agent called ${args.to}` };
      return { value: { to: agent.name, agentId: agent.agentId, delivery: 'newTurn', note: `${agent.name} was idle and started a turn to read this.` } };
    }
    case 'agent_spawn':
      return {
        value: {
          name: 'Fern',
          agentId: 'agent-eval-spawned',
          cli: args.cli ?? 'claude',
          reportsBack: true,
          note: 'Fern is starting in its own tab and gets your prompt once it is ready. Each time its turn ends, its final answer comes to you as a message.',
        },
      };
    case 'agent_read':
      return { value: { name: args.agent, turns: [], status: 'idle' } };
    case 'agent_sessions':
      return { value: { sessions: [] } };
    case 'agent_wait':
      return { value: { events: [], cursor: String(++state.cursor), timedOut: true, note: 'Nothing you wait for has happened yet. Wait again with this cursor, or carry on: an agent you started reports to you by itself.' } };
    case 'who_wrote':
      return { value: { target: 'file', path: args.target, commits: [], owners: [{ kind: 'person', name: 'the person', action: 'ask the person' }], note: '' } };
    case 'git_commit':
      return {
        value: {
          commit: '0f3c2a91d5e8b7c4a6f1e2d3c4b5a69788776655',
          branch: 'refs/heads/main',
          files: (args.paths ?? ['changed file']).map((path) => ({ path, added: 3, removed: 1 })),
          left: [],
          ...(args.land ? { landed: true } : {}),
        },
      };
    case 'ask':
      return {
        value: {
          questionId: 'question-eval-1',
          askedOf: fixture.lead ? fixture.lead : 'the person',
          note: args.blocked === false ? 'Carry on with your assumption; a message tells you if it is wrong.' : 'Asked. End your turn now; the answer comes to you as a message.',
        },
      };
    case 'app_console':
      return { value: { recorded: 0, matched: 0, messages: [] } };
    case 'workspace_inspect':
      return { value: { project, roots: [project], tasks: [] } };
    case 'ui_open':
      return { value: { opened: args.kind ?? 'file' } };
    case 'board_list':
      return { value: { cards: [] } };
    case 'board_add':
      return { value: { card: 1, title: args.title, status: 'open' } };
    case 'sim_devices':
      return { value: { devices: [{ udid: 'SIM-EVAL-1', name: 'iPhone 17', state: 'Booted', runtime: 'iOS 26.0' }] } };
    case 'sim_screenshot':
      return { value: { data: GRAY_PNG, mimeType: 'image/png', title: 'iPhone 17', url: '' } };
    case 'sim_state':
      return { value: { app: 'com.example.app', elements: elementsText(fixture.sim ?? BLANK_PAGE) } };
    case 'jira_search':
      return { value: { issues: [], next: null } };
    case 'jira_issue':
      return { value: { key: args.key ?? 'PROJ-1', summary: 'Ticket', status: 'To Do', description: '' } };
    case 'jira_comment':
    case 'jira_edit':
    case 'jira_transition':
    case 'jira_link':
    case 'jira_worklog':
      return { value: { ok: true, key: args.key ?? args.issue ?? null } };
    case 'jira_create':
      return { value: { key: 'PROJ-101', url: 'https://example.atlassian.net/browse/PROJ-101' } };
    default:
      return { value: { ok: true } };
  }
}
