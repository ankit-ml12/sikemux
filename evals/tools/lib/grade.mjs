import { callMatches, describe, toolMatches, valueMatches } from './match.mjs';

const bash = (pattern) => ({ tool: ['Bash', 'BashOutput'], args: { command: pattern } });

export const BANNED = {
  atlassian: { tool: 'atlassian.*', why: 'used the Atlassian MCP instead of jira_*' },
  'claude-browser': { tool: 'Claude_Browser.*', why: "used Claude's own browser instead of the desk" },
  'send-message': { tool: ['SendMessage', 'ListAgents'], why: "used the CLI's own agent messaging instead of agent_*" },
  'raw-git-commit': { ...bash('/\\bgit\\s+(?:-C\\s+\\S+\\s+)?commit\\b/'), why: 'ran git commit instead of git_commit' },
  'git-sweep': { ...bash('/\\bgit\\s+(?:add\\s+(?:-A|--all|\\.(?:\\s|$))|stash\\b|reset\\s+--hard)/m'), why: 'ran git add -A, git stash or git reset --hard' },
  gh: { ...bash('/(?:^|[\\s;&|(])gh\\s+(?:pr|run|api|issue|repo|workflow|release|search)\\b/m'), why: 'used the gh CLI' },
  'sleep-poll': { ...bash('/(?:^|[;&|]\\s*)sleep\\s+\\d+(?:\\.\\d+)?\\s*(?:;|&&|$)/m'), why: 'waited by sleeping' },
};

const PARALLEL = [
  { tool: 'browser_*', key: (call) => call.args.tabId ?? 'current tab' },
  { tool: 'sim_*', key: () => 'simulator' },
  { tool: 'git_commit', key: () => 'commit' },
  { tool: ['Edit', 'Write', 'MultiEdit', 'NotebookEdit'], key: (call) => call.args.file_path ?? call.args.notebook_path },
];

export function grade(testCase, outcome) {
  const spec = testCase.grade ?? {};
  const calls = outcome.calls;
  const failures = [];

  const allowed = new Set(spec.allow ?? []);
  for (const [id, banned] of Object.entries(BANNED)) {
    if (allowed.has(id)) continue;
    const hit = calls.find((call) => callMatches(banned, call));
    if (hit) failures.push({ check: id, why: banned.why, call: brief(hit) });
  }

  if (!allowed.has('parallel')) {
    const batches = Map.groupBy(calls, (call) => call.batch);
    for (const batch of batches.values()) {
      for (const rule of PARALLEL) {
        const seen = new Map();
        for (const call of batch.filter((one) => toolMatches(rule.tool, one.tool))) {
          const key = rule.key(call);
          if (seen.has(key)) {
            failures.push({ check: 'parallel', why: `called ${seen.get(key).tool} and ${call.tool} at once on ${key}` });
            break;
          }
          seen.set(key, call);
        }
      }
    }
  }

  let cursor = 0;
  for (const matcher of spec.expect ?? []) {
    const from = spec.order ? cursor : 0;
    const index = calls.findIndex((call, at) => at >= from && callMatches(matcher, call));
    if (index < 0) {
      failures.push({ check: 'expect', why: `never called ${describe(matcher)}${spec.order && from ? ' after the calls before it' : ''}` });
    } else {
      cursor = index + 1;
    }
  }

  for (const matcher of spec.forbid ?? []) {
    const hit = calls.find((call) => callMatches(matcher, call));
    if (hit) failures.push({ check: 'forbid', why: matcher.why ?? `called ${describe(matcher)}`, call: brief(hit) });
  }

  for (const limit of spec.limit ?? []) {
    const count = calls.filter((call) => callMatches(limit, call)).length;
    if (count > limit.max) failures.push({ check: 'limit', why: `called ${describe(limit)} ${count} times, more than ${limit.max}` });
  }

  if (spec.first) {
    const relevant = calls.filter((call) => !['ToolSearch', 'guide', 'TodoWrite'].includes(call.tool));
    const first = relevant[0];
    if (!first || !callMatches(spec.first, first)) {
      failures.push({ check: 'first', why: `first call was ${first ? brief(first) : 'none'}, not ${describe(spec.first)}` });
    }
  }

  if (spec.reply && !valueMatches(spec.reply, outcome.reply ?? '')) {
    failures.push({ check: 'reply', why: spec.replyWhy ?? `final reply did not match ${spec.reply}` });
  }

  return failures;
}

export function brief(call) {
  const args = JSON.stringify(call.args);
  return `${call.tool} ${args.length > 160 ? `${args.slice(0, 160)}…` : args}`;
}
