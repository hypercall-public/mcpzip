// @ts-check
const { themes: prismThemes } = require("prism-react-renderer");

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "mcpzip",
  tagline: "Aggregate hundreds of MCP tools behind 3 meta-tools",
  favicon: "img/favicon.ico",

  url: "https://hypercall-public.github.io",
  baseUrl: "/mcpzip/",

  organizationName: "hypercall-public",
  projectName: "mcpzip",

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "warn",

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: "./sidebars.js",
          editUrl: "https://github.com/hypercall-public/mcpzip/tree/main/website/",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      colorMode: {
        defaultMode: "dark",
        respectPrefersColorScheme: true,
      },
      navbar: {
        title: "mcpzip",
        logo: {
          alt: "mcpzip logo",
          src: "img/logo.svg",
        },
        items: [
          {
            type: "docSidebar",
            sidebarId: "docsSidebar",
            position: "left",
            label: "Docs",
          },
          {
            href: "https://hypercall.xyz",
            label: "Hypercall",
            position: "right",
          },
          {
            href: "https://github.com/hypercall-public/mcpzip",
            label: "GitHub",
            position: "right",
          },
        ],
      },
      footer: {
        style: "dark",
        links: [
          {
            title: "Docs",
            items: [
              { label: "Getting Started", to: "/docs/getting-started" },
              { label: "Configuration", to: "/docs/configuration" },
              { label: "Architecture", to: "/docs/architecture" },
            ],
          },
          {
            title: "Community",
            items: [
              {
                label: "GitHub",
                href: "https://github.com/hypercall-public/mcpzip",
              },
              {
                label: "Issues",
                href: "https://github.com/hypercall-public/mcpzip/issues",
              },
            ],
          },
        ],
        copyright: `Copyright ${new Date().getFullYear()} <a href="https://hypercall.xyz" style="color: #5CF53D;">Hypercall</a>. Built with Docusaurus.`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ["rust", "json", "bash", "toml"],
      },
    }),
};

module.exports = config;
