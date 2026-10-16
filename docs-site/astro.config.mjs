import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://docs.sikemux.com",
  integrations: [
    starlight({
      title: "Sikemux",
      description: "Projects, terminals and coding agents in one keyboard-first window.",
      logo: { src: "./src/assets/mark.svg" },
      favicon: "/favicon.png",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/nodelike/sikemux" }],
      editLink: { baseUrl: "https://github.com/nodelike/sikemux/edit/main/docs-site/" },
      customCss: ["./src/styles/theme.css"],
      sidebar: [
        { label: "Getting started", items: [{ autogenerate: { directory: "getting-started" } }] },
        { label: "Agents", items: [{ autogenerate: { directory: "agents" } }] },
        { label: "Browser", items: [{ autogenerate: { directory: "browser" } }] },
        { label: "Reference", items: [{ autogenerate: { directory: "reference" } }] },
      ],
    }),
  ],
});
