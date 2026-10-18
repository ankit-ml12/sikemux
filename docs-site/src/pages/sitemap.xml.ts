import type { APIRoute } from "astro";
import { source } from "@/lib/source";

export const GET: APIRoute = ({ site }) => {
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${source
  .getPages()
  .map((page) => `  <url><loc>${new URL(page.url, site)}</loc></url>`)
  .join("\n")}
</urlset>
`;
  return new Response(body, { headers: { "Content-Type": "application/xml" } });
};
