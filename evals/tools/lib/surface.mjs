import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO = `${join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..')}/`;

export const SIGNED_IN_PLUGINS = ['aws', 'jira', 'rundeck', 'signoz', 'vercel'];

export function loadSurface(plugins = SIGNED_IN_PLUGINS) {
  const manifest = JSON.parse(readFileSync(join(REPO, 'browser/tools.json'), 'utf8'));
  const guide = readFileSync(join(REPO, 'browser', manifest.guide.file), 'utf8');
  const byMethod = new Map(manifest.tools.map((tool) => [tool.method, tool.name]));
  const pluginTools = plugins.flatMap((folder) => {
    const path = join(REPO, 'src-tauri/plugins', folder, 'manifest.json');
    if (!existsSync(path)) throw new Error(`no plugin called ${folder}`);
    const plugin = JSON.parse(readFileSync(path, 'utf8'));
    return (plugin.tools ?? []).map((tool) => ({
      plugin: plugin.id,
      name: tool.name,
      method: tool.method,
      description: tool.description,
      properties: tool.properties ?? {},
      required: tool.required ?? [],
    }));
  });
  return { manifest, guide, byMethod, pluginTools };
}

export function allPlugins() {
  return readdirSync(join(REPO, 'src-tauri/plugins'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
}
