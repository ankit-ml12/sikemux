import { spawn } from 'node:child_process';
import { createWriteStream, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { REPO } from './surface.mjs';

const EVAL = join(REPO, 'evals/tools');
const SERVER_PREFIX = /^mcp__([^_]+(?:[-_][^_]+)*?)__(.+)$/;

export const MODELS = {
  sonnet: { runner: 'claude', id: 'claude-sonnet-5-5' },
  opus: { runner: 'claude', id: 'claude-opus-5-5' },
  haiku: { runner: 'claude', id: 'claude-haiku-5-5' },
  deepseek: { runner: 'omp', id: 'deepseek-ai/DeepSeek-V4.1-Flash-ultra-chat' },
};

export function mcpServers({ host, endpoint, repo, dir, rivals = [] }) {
  const servers = {
    'sikemux-tools': {
      command: host,
      args: ['--tools-mcp'],
      env: { SIKEMUX_TOOLS_AGENT_ID: 'agent-eval-self', SIKEMUX_CLI_ENDPOINT: endpoint, SIKEMUX_PROJECT: repo },
    },
  };
  for (const rival of rivals) {
    servers[rival] = { command: process.execPath, args: [join(EVAL, 'lib/rival.mjs'), join(EVAL, `rivals/${rival.toLowerCase()}.json`), dir] };
  }
  return { mcpServers: servers };
}

export function run({ command, args, cwd, env, dir, timeoutMs, profile = join(dir, 'sandbox.sb') }) {
  return new Promise((resolve) => {
    const out = createWriteStream(join(dir, 'transcript.jsonl'));
    const err = createWriteStream(join(dir, 'stderr.log'));
    const child = spawn('/usr/bin/sandbox-exec', ['-f', profile, command, ...args], { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
    child.stdout.pipe(out);
    child.stderr.pipe(err);
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill('SIGTERM');
      setTimeout(() => child.kill('SIGKILL'), 5000);
    }, timeoutMs);
    child.on('close', (code) => {
      clearTimeout(timer);
      out.end(() => resolve({ code, timedOut }));
    });
  });
}

export async function runClaude({ model, search, dir, sandbox, servers, prompt, maxTurns, timeoutMs }) {
  const config = join(dir, 'mcp.json');
  writeFileSync(config, JSON.stringify(servers, null, 1));
  const args = [
    '-p',
    prompt,
    '--model',
    model,
    '--output-format',
    'stream-json',
    '--verbose',
    `--mcp-config=${config}`,
    '--strict-mcp-config',
    '--permission-mode',
    'bypassPermissions',
    '--max-turns',
    String(maxTurns),
    '--no-session-persistence',
  ];
  const env = {
    ...sandbox.env,
    ENABLE_TOOL_SEARCH: search ? 'true' : 'false',
    ENABLE_CLAUDEAI_MCP_SERVERS: 'false',
    DISABLE_AUTOUPDATER: '1',
    CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY: '1',
  };
  const exit = await run({ command: join(process.env.HOME, '.local/bin/claude'), args, cwd: sandbox.repo, env, dir, timeoutMs });
  return { exit, ...readClaude(join(dir, 'transcript.jsonl')) };
}

export function canonical(name) {
  const server = name.match(SERVER_PREFIX);
  if (!server) return name;
  return server[1] === 'sikemux-tools' ? server[2] : `${server[1]}.${server[2]}`;
}

export function readClaude(path) {
  const calls = [];
  const byId = new Map();
  let reply = '';
  let result = null;
  let init = null;
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      continue;
    }
    if (event.type === 'system' && event.subtype === 'init') init = event;
    if (event.type === 'assistant') {
      for (const block of event.message?.content ?? []) {
        if (block.type === 'tool_use') {
          const call = {
            id: block.id,
            tool: canonical(block.name),
            args: block.input ?? {},
            batch: event.message.id,
            sub: Boolean(event.parent_tool_use_id),
          };
          calls.push(call);
          byId.set(block.id, call);
        }
        if (block.type === 'text' && !event.parent_tool_use_id) reply = block.text;
      }
    }
    if (event.type === 'user') {
      for (const block of event.message?.content ?? []) {
        if (block.type !== 'tool_result') continue;
        const call = byId.get(block.tool_use_id);
        if (!call) continue;
        call.error = Boolean(block.is_error);
        call.result = resultText(block.content).slice(0, 1500);
      }
    }
    if (event.type === 'result') result = event;
  }
  const usage = result?.usage ?? {};
  return {
    calls,
    reply: result?.result ?? reply,
    failed: result ? result.is_error && result.subtype !== 'error_max_turns' : true,
    stop: result?.subtype ?? 'no result',
    turns: result?.num_turns ?? 0,
    cost: result?.total_cost_usd ?? 0,
    tokens: {
      input: (usage.input_tokens ?? 0) + (usage.cache_read_input_tokens ?? 0) + (usage.cache_creation_input_tokens ?? 0),
      cached: usage.cache_read_input_tokens ?? 0,
      output: usage.output_tokens ?? 0,
    },
    toolCount: init?.tools?.length ?? 0,
  };
}

function resultText(content) {
  if (typeof content === 'string') return content;
  return (content ?? []).map((block) => (block.type === 'text' ? block.text : `[${block.type}]`)).join('\n');
}
