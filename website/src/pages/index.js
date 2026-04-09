import React from "react";
import clsx from "clsx";
import Link from "@docusaurus/Link";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import Layout from "@theme/Layout";
import styles from "./index.module.css";

const features = [
  {
    title: "3 Tools, Not 300",
    description:
      "Replace hundreds of tool schemas in your context window with just search_tools, describe_tool, and execute_tool. Claude finds what it needs on demand.",
  },
  {
    title: "Smart Search",
    description:
      "Keyword matching for speed, optional Gemini-powered semantic search for natural language queries like \"send someone a message on slack\".",
  },
  {
    title: "Instant Startup",
    description:
      "Serves from a disk-cached tool catalog immediately. Upstream servers connect and refresh in the background \u2014 no waiting.",
  },
  {
    title: "All Transports",
    description:
      "stdio, HTTP (Streamable HTTP), and SSE. OAuth 2.1 with PKCE for authenticated HTTP servers. Reuses mcp-remote tokens.",
  },
  {
    title: "One Command Migration",
    description:
      "Already have MCP servers in Claude Code? Run mcpzip migrate and you're done. It reads your config and rewires everything.",
  },
  {
    title: "~5MB Binary",
    description:
      "Single static Rust binary. No Node.js, no Python, no runtime dependencies. Fast, small, reliable.",
  },
];

function Feature({ title, description }) {
  return (
    <div className={clsx("col col--4")}>
      <div className="feature-card text--center padding-horiz--md margin-bottom--lg">
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
    </div>
  );
}

function HomepageHeader() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <header className={clsx("hero hero--primary", styles.heroBanner)}>
      <div className="container">
        <h1 className="hero__title">{siteConfig.title}</h1>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <div className={styles.buttons}>
          <Link
            className="button button--lg"
            style={{
              background: "#5CF53D",
              color: "#050507",
              border: "none",
              fontWeight: 700,
            }}
            to="/docs/getting-started"
          >
            Get Started
          </Link>
          <Link
            className="button button--outline button--lg margin-left--md"
            style={{
              color: "rgba(255,255,255,0.85)",
              borderColor: "rgba(255,255,255,0.12)",
            }}
            to="https://github.com/hypercall-public/mcpzip"
          >
            GitHub
          </Link>
        </div>
      </div>
    </header>
  );
}

function HowItWorks() {
  return (
    <section className={styles.howItWorks}>
      <div className="container">
        <h2 className="text--center margin-bottom--lg">How It Works</h2>
        <div className="row">
          <div className="col col--8 col--offset-2">
            <div className={styles.diagram}>
              <pre>{`Claude Code                mcpzip                    MCP Servers
    |                        |                           |
    |-- search_tools ------->|                           |
    |   "send a message"     |-- (keyword + LLM search)  |
    |<-- results ------------|                           |
    |   slack__send_message   |                           |
    |                        |                           |
    |-- execute_tool ------->|                           |
    |   slack__send_message   |-- tools/call ----------->|
    |   {channel, text}      |<-- result ----------------|
    |<-- result -------------|                           |`}</pre>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function BuiltBy() {
  return (
    <section style={{ padding: "2rem 0 3rem", textAlign: "center" }}>
      <div className="container">
        <p style={{ fontSize: "1.1rem", opacity: 0.7 }}>
          Built by{" "}
          <a
            href="https://hypercall.xyz"
            style={{ color: "#5CF53D", fontWeight: 600 }}
          >
            Hypercall
          </a>
        </p>
      </div>
    </section>
  );
}

export default function Home() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <Layout title="Home" description={siteConfig.tagline}>
      <HomepageHeader />
      <main>
        <section className={styles.features}>
          <div className="container">
            <div className="row">
              {features.map((props, idx) => (
                <Feature key={idx} {...props} />
              ))}
            </div>
          </div>
        </section>
        <HowItWorks />
        <BuiltBy />
      </main>
    </Layout>
  );
}
