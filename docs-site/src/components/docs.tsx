import { navigate } from "astro:transitions/client";
import type { Root } from "fumadocs-core/page-tree";
import type { AstroProviderProps } from "fumadocs-core/framework/astro";
import { DocsLayout } from "fumadocs-ui/layouts/docs";
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
  EditOnGitHub,
  MarkdownCopyButton,
} from "fumadocs-ui/layouts/docs/page";
import { RootProvider } from "fumadocs-ui/provider/astro";
import type { TOCItemType } from "fumadocs-core/toc";
import type { ReactNode } from "react";
import { Hero } from "./hero";
import { Wordmark } from "./logo";
import Search from "./search";

export function Docs(props: {
  tree: Root;
  pathname: string;
  params: AstroProviderProps["params"];
  title: string;
  description: string;
  toc: TOCItemType[];
  markdownUrl: string;
  editUrl: string;
  landing: boolean;
  children: ReactNode;
}) {
  return (
    <RootProvider
      pathname={props.pathname}
      params={props.params}
      navigate={navigate}
      theme={{ enabled: false }}
      search={{ SearchDialog: Search }}
    >
      <DocsLayout
        tree={props.tree}
        themeSwitch={{ enabled: false }}
        githubUrl="https://github.com/nodelike/sikemux"
        nav={{
          title: <Wordmark />,
        }}
      >
        {props.landing ? (
          <DocsPage full breadcrumb={{ enabled: false }} footer={{ enabled: false }} tableOfContent={{ enabled: false }}>
            <Hero title={props.title} description={props.description} />
            <DocsBody className="landing-body">{props.children}</DocsBody>
          </DocsPage>
        ) : (
          <DocsPage toc={props.toc}>
            <DocsTitle>{props.title}</DocsTitle>
            <DocsDescription>{props.description}</DocsDescription>
            <div className="page-actions">
              <MarkdownCopyButton markdownUrl={props.markdownUrl} />
              <EditOnGitHub href={props.editUrl} />
            </div>
            <DocsBody>{props.children}</DocsBody>
          </DocsPage>
        )}
      </DocsLayout>
    </RootProvider>
  );
}
