import type { APIRoute, GetStaticPaths } from "astro";
import { getCollection, type CollectionEntry } from "astro:content";

/** Every page again as plain markdown, at its own address with `.md` on the end. */
export const getStaticPaths: GetStaticPaths = async () =>
  (await getCollection("docs")).map((doc) => ({ params: { slug: doc.id }, props: { doc } }));

export const GET: APIRoute<{ doc: CollectionEntry<"docs"> }> = ({ props: { doc } }) => {
  const body = (doc.body ?? "").replace(/<!--[\s\S]*?-->\n*/g, "").trim();
  return new Response(`# ${doc.data.title}\n\n${body}\n`, { headers: { "Content-Type": "text/markdown; charset=utf-8" } });
};
