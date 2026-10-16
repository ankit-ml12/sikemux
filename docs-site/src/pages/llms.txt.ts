import type { APIRoute } from "astro";
import { getCollection } from "astro:content";

const SECTIONS = ["getting-started", "agents", "browser", "reference"];

/** The pages as plain markdown, listed the way agents look for them: https://llmstxt.org */
export const GET: APIRoute = async ({ site }) => {
  const docs = await getCollection("docs");
  const sections = SECTIONS.map((section) => {
    const pages = docs
      .filter((doc) => doc.id.startsWith(`${section}/`))
      .sort((a, b) => (a.data.sidebar.order ?? 0) - (b.data.sidebar.order ?? 0))
      .map((doc) => `- [${doc.data.title}](${new URL(`${doc.id}.md`, site)}): ${doc.data.description ?? ""}`);
    const title = section.replace("-", " ").replace(/^./, (first) => first.toUpperCase());
    return `## ${title}\n\n${pages.join("\n")}`;
  });
  const body = `# Sikemux\n\n> Projects, terminals and coding agents in one keyboard-first window.\n\n${sections.join("\n\n")}\n`;
  return new Response(body, { headers: { "Content-Type": "text/plain; charset=utf-8" } });
};
