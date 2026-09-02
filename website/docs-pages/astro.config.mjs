import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

export default defineConfig({
  site: "https://docs.tohseno.com",
  integrations: [
    starlight({
      title: "Tohseno",
      description:
        "Learn how Tohseno connects an iPhone app to its source, history, builder, and exact path back to the phone.",
      favicon: "/favicon.png",
      customCss: ["./src/styles/starlight.css"],
      components: {
        PageTitle: "./src/components/PageTitle.astro",
      },
      editLink: {
        baseUrl:
          "https://github.com/jpfraneto/tohseno/edit/main/website/docs-pages/src/content/docs/",
      },
      social: [
        {
          icon: "github",
          label: "Tohseno on GitHub",
          href: "https://github.com/jpfraneto/tohseno",
        },
      ],
      head: [
        {
          tag: "link",
          attrs: {
            rel: "alternate",
            type: "text/plain",
            href: "/llms.txt",
            title: "Tohseno documentation for AI agents",
          },
        },
        {
          tag: "meta",
          attrs: {
            property: "og:image",
            content: "https://tohseno.com/og.png",
          },
        },
        {
          tag: "meta",
          attrs: { name: "theme-color", content: "#f2efe7" },
        },
      ],
      lastUpdated: true,
      sidebar: [
        { label: "Home", link: "/" },
        {
          label: "Start here",
          items: [
            "guide/start/what-is-tohseno",
            "guide/start/requirements",
            "guide/start/install-and-onboard",
          ],
        },
        {
          label: "Make & evolve",
          items: [
            "guide/start/create-an-app",
            "guide/start/adopt-an-app",
            "guide/start/evolve-an-app",
            "guide/product/mental-model",
            "guide/product/mac-app",
            "guide/product/companion",
            "guide/product/app-workspace",
          ],
        },
        {
          label: "Share",
          items: [
            "guide/product/registry",
            "guide/product/ship-claim-update",
            "guide/architecture/person-to-person-network",
          ],
        },
        {
          label: "How it works",
          collapsed: true,
          items: [
            "guide/architecture/overview",
            "guide/architecture/factory",
            "guide/architecture/command-lifecycle",
            "guide/architecture/apple-delivery",
            "guide/architecture/persistence",
            "guide/architecture/managed-compute",
          ],
        },
        {
          label: "Protocol",
          collapsed: true,
          items: [
            "guide/protocol/authority-and-scope",
            "guide/protocol/identities",
            "guide/protocol/shots-evolutions-and-lineage",
            "guide/protocol/commitments-and-signatures",
            "guide/protocol/generation-0-8",
            "guide/protocol/public-witness-and-claims",
            "guide/protocol/conformance",
          ],
        },
        {
          label: "Trust & privacy",
          collapsed: true,
          items: [
            "guide/security/trust-boundaries",
            "guide/security/private-and-public-data",
            "guide/security/fail-closed-rules",
            "guide/security/source-safety",
          ],
        },
        {
          label: "Operate & develop",
          collapsed: true,
          items: [
            "guide/operations/repository-map",
            "guide/operations/build-and-test",
            "guide/operations/release-and-activation",
            "guide/operations/troubleshooting",
          ],
        },
        {
          label: "Reference",
          collapsed: true,
          items: [
            "guide/reference/current-status",
            "guide/reference/states-and-errors",
            "guide/reference/files-and-directories",
            "guide/reference/glossary",
            "guide/reference/source-of-truth",
          ],
        },
      ],
    }),
  ],
});
