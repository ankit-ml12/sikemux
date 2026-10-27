import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { JSDOM, VirtualConsole } from 'jsdom';

const [path, page, script] = process.argv.slice(2);
const messages = [];
const sink = new VirtualConsole();
for (const level of ['log', 'info', 'warn', 'error', 'debug']) sink.on(level, (...parts) => messages.push({ level, text: parts.join(' ') }));
sink.on('jsdomError', (error) => messages.push({ level: 'error', text: error.message }));

const dom = new JSDOM(readFileSync(path, 'utf8'), { url: pathToFileURL(page).href, runScripts: 'dangerously', pretendToBeVisual: true, virtualConsole: sink });
await new Promise((resolve) => (dom.window.document.readyState === 'complete' ? resolve() : dom.window.addEventListener('load', resolve)));

let result;
if (script) {
  const run = (body) => dom.window.eval(`(async () => {\n${body}\n})()`);
  try {
    result = await run(`return (${script}\n);`);
  } catch (error) {
    try {
      result = error instanceof dom.window.SyntaxError ? await run(script) : { error: String(error?.message ?? error) };
    } catch (inner) {
      result = { error: String(inner?.message ?? inner) };
    }
  }
}
process.stdout.write(JSON.stringify({ script: result ?? null, messages }));
dom.window.close();
