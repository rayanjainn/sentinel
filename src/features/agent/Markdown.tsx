import { motion } from "motion/react";
import { memo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { usePrefersReducedMotion } from "../../lib/motion";

// Links are shown, not followed: navigating would replace the app's own webview.
const components: Components = {
  p: ({ children }) => <p className="my-2 first:mt-0 last:mb-0">{children}</p>,
  ul: ({ children }) => <ul className="my-2 list-disc space-y-1 pl-5 marker:text-fg-subtle">{children}</ul>,
  ol: ({ children }) => <ol className="my-2 list-decimal space-y-1 pl-5 marker:text-fg-subtle">{children}</ol>,
  li: ({ children }) => <li className="pl-0.5">{children}</li>,
  strong: ({ children }) => <strong className="font-semibold text-fg">{children}</strong>,
  h1: ({ children }) => <h4 className="mb-1 mt-3 text-[13px] font-semibold text-fg">{children}</h4>,
  h2: ({ children }) => <h4 className="mb-1 mt-3 text-[13px] font-semibold text-fg">{children}</h4>,
  h3: ({ children }) => <h4 className="mb-1 mt-3 text-[13px] font-semibold text-fg">{children}</h4>,
  blockquote: ({ children }) => <blockquote className="my-2 border-l-2 border-line-strong pl-3 text-fg-muted">{children}</blockquote>,
  code: ({ children }) => (
    <code className="num rounded-[4px] bg-sunken px-1 py-px text-[12px] text-fg">{children}</code>
  ),
  pre: ({ children }) => (
    <pre className="my-2 overflow-x-auto rounded-[5px] border border-line bg-sunken p-2.5 text-[12px] [&_code]:bg-transparent [&_code]:p-0">
      {children}
    </pre>
  ),
  a: ({ children, href }) => (
    <span className="text-signal underline decoration-signal/40 underline-offset-2" title={href}>
      {children}
    </span>
  ),
  table: ({ children }) => (
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-[12px]">{children}</table>
    </div>
  ),
  th: ({ children }) => <th className="border-b border-line-strong px-2 py-1 text-left font-medium text-fg-muted">{children}</th>,
  td: ({ children }) => <td className="border-b border-line px-2 py-1 align-top">{children}</td>,
  hr: () => <hr className="my-3 border-line" />,
};

function Caret() {
  const reduced = usePrefersReducedMotion();
  return (
    <motion.span
      aria-hidden
      className="ml-0.5 inline-block h-[1.05em] w-[2px] translate-y-[3px] rounded-full bg-signal"
      animate={reduced ? undefined : { opacity: [1, 0.15, 1] }}
      transition={{ duration: 1.1, repeat: Infinity, ease: "easeInOut" }}
    />
  );
}

export const Markdown = memo(function Markdown({ text, streaming = false }: { text: string; streaming?: boolean }) {
  return (
    <div className="selectable break-words text-[13px] leading-relaxed text-fg">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {text}
      </ReactMarkdown>
      {streaming && <Caret />}
    </div>
  );
});
