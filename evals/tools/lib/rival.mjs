import { readFileSync, appendFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
import { valueMatches, toolMatches } from './match.mjs';

const [specPath, caseDir] = process.argv.slice(2);
const spec = JSON.parse(readFileSync(specPath, 'utf8'));
const fixture = JSON.parse(readFileSync(`${caseDir}/fixture.json`, 'utf8'));
const rules = (fixture.answers ?? []).filter((rule) => [rule.tool].flat().some((name) => name.startsWith(`${spec.name}.`)));

const emit = (message) => process.stdout.write(`${JSON.stringify(message)}\n`);

function answer(name, args) {
  const qualified = `${spec.name}.${name}`;
  const rule = rules.find((candidate) => toolMatches(candidate.tool, qualified) && (!candidate.args || valueMatches(candidate.args, args)));
  appendFileSync(`${caseDir}/rival.jsonl`, `${JSON.stringify({ at: Date.now(), tool: qualified, args })}\n`);
  if (rule?.error) return { content: [{ type: 'text', text: rule.error }], isError: true };
  const reply = rule?.reply ?? (name === 'getAccessibleAtlassianResources' ? [{ id: 'cloud-eval-1', url: 'https://example.atlassian.net', name: 'example' }] : { ok: true });
  return { content: [{ type: 'text', text: JSON.stringify(reply) }], isError: false };
}

createInterface({ input: process.stdin }).on('line', (line) => {
  if (!line.trim()) return;
  const message = JSON.parse(line);
  if (message.id === undefined) return;
  switch (message.method) {
    case 'initialize':
      return emit({
        jsonrpc: '2.0',
        id: message.id,
        result: {
          protocolVersion: message.params?.protocolVersion ?? '2025-06-18',
          capabilities: { tools: {} },
          serverInfo: { name: spec.name, version: '1.0.0' },
          instructions: spec.instructions,
        },
      });
    case 'tools/list':
      return emit({ jsonrpc: '2.0', id: message.id, result: { tools: spec.tools } });
    case 'tools/call':
      return emit({ jsonrpc: '2.0', id: message.id, result: answer(message.params.name, message.params.arguments ?? {}) });
    case 'ping':
      return emit({ jsonrpc: '2.0', id: message.id, result: {} });
    default:
      return emit({ jsonrpc: '2.0', id: message.id, error: { code: -32601, message: 'Method not found' } });
  }
});
