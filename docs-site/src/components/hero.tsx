import { Check, Copy } from "lucide-react";
import { useState } from "react";

const INSTALL = "curl -fsSL https://sikemux.com/install | sh";

export function Hero({ title, description }: { title: string; description: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <header className="hero">
      <p className="hero-label">Documentation</p>
      <h1>{title}</h1>
      <p className="hero-lede">{description}</p>
      <div className="hero-actions">
        <a className="hero-btn hero-btn-primary" href="/getting-started/install/">
          Get started
        </a>
        <button
          type="button"
          className="hero-install"
          aria-label="Copy the install command"
          data-copied={copied || undefined}
          onClick={() => {
            void navigator.clipboard.writeText(INSTALL);
            setCopied(true);
            setTimeout(() => setCopied(false), 1600);
          }}
        >
          <span className="hero-prompt" aria-hidden="true">
            $
          </span>
          <code>{INSTALL}</code>
          {copied ? <Check size={14} /> : <Copy size={14} />}
        </button>
      </div>
      <button
        type="button"
        className="hero-install"
        aria-label="Copy the install command"
        onClick={() => {
          void navigator.clipboard.writeText(INSTALL);
          setCopied(true);
          setTimeout(() => setCopied(false), 1400);
        }}
      >
        <span className="hero-prompt">$</span>
        <code>{INSTALL}</code>
        <span className="hero-copy">{copied ? "Copied" : "Copy"}</span>
      </button>
    </header>
  );
}
