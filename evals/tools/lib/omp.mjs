import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { run } from './runners.mjs';

const PROVIDER = 'sikemux-lithosai';
const KEY_ENV = 'SIKEMUX_MODEL_PROVIDER_KEY';
const BASE_URL = process.env.TOOL_EVAL_LITHOS_URL ?? 'https://api.lithosai.cloud/v1';
// LithosAI's blended price for DeepSeek V4.1 Flash, in dollars per million tokens. OMP reports no cost for a provider the person adds.
export const LITHOS_PRICE_PER_MILLION = 0.09;
const EXTENSION = `export default function (pi) {
  const provider = process.env.SIKEMUX_OMP_PROVIDER;
  if (provider) {
    const { id, ...config } = JSON.parse(provider);
    pi.registerProvider(id, config);
  }
}
`;

export async function runOmp({ model, dir, sandbox, servers, prompt, maxTurns, timeoutMs }) {
  const key = process.env.TOOL_EVAL_LITHOS_KEY_FILE ? readFileSync(process.env.TOOL_EVAL_LITHOS_KEY_FILE, 'utf8').trim() : process.env.TOOL_EVAL_LITHOS_KEY;
  if (!key) throw new Error('DeepSeek runs need TOOL_EVAL_LITHOS_KEY_FILE, a file holding the LithosAI key');
  const extension = join(dir, 'omp-model-provider.js');
  writeFileSync(extension, EXTENSION);
  writeFileSync(join(sandbox.repo, '.mcp.json'), JSON.stringify(servers, null, 1));
  const entry = { id: model, input: ['text'] };
  if (model.toLowerCase().includes('deepseek')) entry.compat = { requiresReasoningContentForToolCalls: true, reasoningContentField: 'reasoning_content' };
  const registered = { id: PROVIDER, baseUrl: BASE_URL, apiKey: KEY_ENV, api: 'openai-completions', models: [entry] };
  const args = ['-p', '--mode', 'json', '-e', extension, '--model', `${PROVIDER}/${model}`, '--no-skills', '--no-session', '--no-title', '--approval-mode', 'yolo', prompt];
  const env = { ...sandbox.env, [KEY_ENV]: key, SIKEMUX_OMP_PROVIDER: JSON.stringify(registered), SIKEMUX_MODEL_PROVIDER: 'lithosai' };
  const exit = await run({ command: join(process.env.HOME, '.bun/bin/omp'), args, cwd: sandbox.repo, env, dir, timeoutMs: Math.min(timeoutMs, maxTurns * 60_000) });
  return { exit, ...readOmp(join(dir, 'transcript.jsonl'), Object.keys(servers.mcpServers)) };
}

const BUILT_IN = { bash: 'Bash', edit: 'Edit', write: 'Write', read: 'Read', web_search: 'WebSearch', task: 'Task', todo: 'TodoWrite' };

function canonicalCall(name, args, servers) {
  const path = String(args.path ?? '');
  if ((name === 'write' || name === 'read') && path.startsWith('xd://mcp__')) {
    const device = path.slice('xd://mcp__'.length);
    const server = servers.find((one) => device.startsWith(`${one.replace(/[^A-Za-z0-9]/g, '_')}_`));
    const tool = server ? device.slice(server.length + 1) : device;
    const qualified = !server || server === 'sikemux-tools' ? tool : `${server}.${tool}`;
    if (name === 'read') return { tool: 'ToolSearch', args: { query: qualified } };
    try {
      return { tool: qualified, args: args.content ? JSON.parse(args.content) : {} };
    } catch {
      return { tool: qualified, args: { unparsed: args.content } };
    }
  }
  if (name === 'read' && /^https?:\/\//.test(path)) return { tool: 'WebFetch', args: { url: path } };
  const renamed = BUILT_IN[name] ?? name;
  return { tool: renamed, args: path && renamed !== 'Bash' ? { ...args, file_path: path } : args };
}

export function readOmp(path, servers) {
  const calls = [];
  const byId = new Map();
  let reply = '';
  let ended = false;
  let turns = 0;
  const tokens = { input: 0, cached: 0, output: 0 };
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim().startsWith('{')) continue;
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      continue;
    }
    if (event.type === 'agent_end') ended = true;
    if (event.type !== 'message_end') continue;
    const message = event.message;
    if (message.role === 'assistant') {
      turns += 1;
      const usage = message.usage ?? {};
      tokens.input += (usage.input ?? 0) + (usage.cacheRead ?? 0) + (usage.cacheWrite ?? 0);
      tokens.cached += usage.cacheRead ?? 0;
      tokens.output += usage.output ?? 0;
      for (const block of message.content ?? []) {
        if (block.type === 'toolCall') {
          const call = { id: block.id, ...canonicalCall(block.name, block.arguments ?? {}, servers), batch: message.responseId ?? message.timestamp, sub: false };
          calls.push(call);
          byId.set(block.id, call);
        }
        if (block.type === 'text' && block.text.trim()) reply = block.text;
      }
    }
    if (message.role === 'toolResult') {
      const call = byId.get(message.toolCallId);
      if (!call) continue;
      call.error = Boolean(message.isError || message.details?.xdev?.inner?.isError);
      call.result = (message.content ?? []).map((block) => (block.type === 'text' ? block.text : `[${block.type}]`)).join('\n').slice(0, 1500);
    }
  }
  const cost = ((tokens.input + tokens.output) / 1_000_000) * LITHOS_PRICE_PER_MILLION;
  return { calls, reply, failed: !ended, stop: ended ? 'end' : 'no end', turns, cost, costEstimated: true, tokens };
}
