import React from 'react';
import { openExternalUrl } from '../api/tauri-bridge';

interface MarkdownViewProps {
  content: string;
  className?: string;
}

/**
 * Safe Markdown renderer without raw HTML or `dangerouslySetInnerHTML`.
 *
 * Renders headings, lists, blockquotes, separators, and bold/italic text
 * safely into React elements (RF-ADMIN-13-3).
 */
export const MarkdownView: React.FC<MarkdownViewProps> = ({ content, className = '' }) => {
  const blocks = parseMarkdownBlocks(content);

  return (
    <div className={`space-y-3 text-fg leading-relaxed text-sm select-text ${className}`}>
      {blocks.map((block, i) => (
        <React.Fragment key={i}>{renderBlock(block)}</React.Fragment>
      ))}
    </div>
  );
};

type Block =
  | { kind: 'h1'; text: string }
  | { kind: 'h2'; text: string }
  | { kind: 'h3'; text: string }
  | { kind: 'h4'; text: string }
  | { kind: 'blockquote'; lines: string[] }
  | { kind: 'ul'; items: string[] }
  | { kind: 'ol'; items: string[] }
  | { kind: 'hr' }
  | { kind: 'p'; text: string };

function parseMarkdownBlocks(raw: string): Block[] {
  const lines = raw.split(/\r?\n/);
  const blocks: Block[] = [];
  let currentParagraph: string[] = [];
  let currentUl: string[] = [];
  let currentOl: string[] = [];
  let currentBlockquote: string[] = [];

  const flushParagraph = () => {
    if (currentParagraph.length > 0) {
      blocks.push({ kind: 'p', text: currentParagraph.join(' ') });
      currentParagraph = [];
    }
  };

  const flushUl = () => {
    if (currentUl.length > 0) {
      blocks.push({ kind: 'ul', items: [...currentUl] });
      currentUl = [];
    }
  };

  const flushOl = () => {
    if (currentOl.length > 0) {
      blocks.push({ kind: 'ol', items: [...currentOl] });
      currentOl = [];
    }
  };

  const flushBlockquote = () => {
    if (currentBlockquote.length > 0) {
      blocks.push({ kind: 'blockquote', lines: [...currentBlockquote] });
      currentBlockquote = [];
    }
  };

  const flushAll = () => {
    flushParagraph();
    flushUl();
    flushOl();
    flushBlockquote();
  };

  for (const line of lines) {
    const trimmed = line.trim();

    if (!trimmed) {
      flushAll();
      continue;
    }

    // Horizontal rule
    if (/^(\*\*\*|---|___)$/.test(trimmed)) {
      flushAll();
      blocks.push({ kind: 'hr' });
      continue;
    }

    // Headings
    if (trimmed.startsWith('# ')) {
      flushAll();
      blocks.push({ kind: 'h1', text: trimmed.slice(2).trim() });
      continue;
    }
    if (trimmed.startsWith('## ')) {
      flushAll();
      blocks.push({ kind: 'h2', text: trimmed.slice(3).trim() });
      continue;
    }
    if (trimmed.startsWith('### ')) {
      flushAll();
      blocks.push({ kind: 'h3', text: trimmed.slice(4).trim() });
      continue;
    }
    if (trimmed.startsWith('#### ')) {
      flushAll();
      blocks.push({ kind: 'h4', text: trimmed.slice(5).trim() });
      continue;
    }

    // Blockquote
    if (trimmed.startsWith('>')) {
      flushParagraph();
      flushUl();
      flushOl();
      currentBlockquote.push(trimmed.replace(/^>\s?/, ''));
      continue;
    }

    // Unordered list
    if (/^[-*]\s+/.test(trimmed)) {
      flushParagraph();
      flushOl();
      flushBlockquote();
      currentUl.push(trimmed.replace(/^[-*]\s+/, ''));
      continue;
    }

    // Ordered list
    if (/^\d+\.\s+/.test(trimmed)) {
      flushParagraph();
      flushUl();
      flushBlockquote();
      currentOl.push(trimmed.replace(/^\d+\.\s+/, ''));
      continue;
    }

    // Regular paragraph continuation
    if (currentUl.length > 0) {
      // Continuation of previous list item
      currentUl[currentUl.length - 1] += ` ${trimmed}`;
    } else if (currentOl.length > 0) {
      currentOl[currentOl.length - 1] += ` ${trimmed}`;
    } else if (currentBlockquote.length > 0) {
      currentBlockquote[currentBlockquote.length - 1] += ` ${trimmed}`;
    } else {
      currentParagraph.push(trimmed);
    }
  }

  flushAll();
  return blocks;
}

function renderBlock(block: Block): React.ReactNode {
  switch (block.kind) {
    case 'h1':
      return (
        <h1 className="text-xl font-bold text-fg pt-2 pb-1 border-b border-default">
          {renderInline(block.text)}
        </h1>
      );
    case 'h2':
      return (
        <h2 className="text-base font-semibold text-fg pt-2 pb-0.5">
          {renderInline(block.text)}
        </h2>
      );
    case 'h3':
      return (
        <h3 className="text-sm font-semibold text-fg pt-1">
          {renderInline(block.text)}
        </h3>
      );
    case 'h4':
      return (
        <h4 className="text-xs font-semibold text-fg">
          {renderInline(block.text)}
        </h4>
      );
    case 'blockquote':
      return (
        <blockquote className="border-l-4 border-accent pl-3 py-1 italic text-muted bg-surface-subtle rounded-r-sm">
          {block.lines.map((line, i) => (
            <p key={i}>{renderInline(line)}</p>
          ))}
        </blockquote>
      );
    case 'ul':
      return (
        <ul className="list-disc list-inside space-y-1 pl-2 text-fg">
          {block.items.map((item, i) => (
            <li key={i} className="leading-relaxed">
              {renderInline(item)}
            </li>
          ))}
        </ul>
      );
    case 'ol':
      return (
        <ol className="list-decimal list-inside space-y-1 pl-2 text-fg">
          {block.items.map((item, i) => (
            <li key={i} className="leading-relaxed">
              {renderInline(item)}
            </li>
          ))}
        </ol>
      );
    case 'hr':
      return <hr className="border-default my-3" />;
    case 'p':
      return <p className="leading-relaxed text-fg">{renderInline(block.text)}</p>;
  }
}

/**
 * Parses bold, italic, code, and links into safe React spans.
 */
function renderInline(text: string): React.ReactNode {
  // Simple token parser for **bold**, *italic*, `code`, and [text](link)
  const regex = /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`|\[[^\]]+\]\([^)]+\))/g;
  const parts = text.split(regex);

  return parts.map((part, index) => {
    if (!part) return null;

    if (part.startsWith('**') && part.endsWith('**') && part.length >= 4) {
      return (
        <strong key={index} className="font-semibold text-fg">
          {part.slice(2, -2)}
        </strong>
      );
    }

    if (part.startsWith('*') && part.endsWith('*') && part.length >= 2) {
      return (
        <em key={index} className="italic">
          {part.slice(1, -1)}
        </em>
      );
    }

    if (part.startsWith('`') && part.endsWith('`') && part.length >= 2) {
      return (
        <code
          key={index}
          className="bg-surface-subtle text-accent px-1 py-0.5 rounded-sm font-mono text-xs border border-default"
        >
          {part.slice(1, -1)}
        </code>
      );
    }

    // Markdown link [label](url)
    const linkMatch = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (linkMatch) {
      const label = linkMatch[1];
      const url = linkMatch[2];
      const isHttps = /^https:\/\//i.test(url);
      if (isHttps) {
        return (
          <a
            key={index}
            href={url}
            title={url}
            onClick={e => {
              e.preventDefault();
              openExternalUrl(url).catch(err => {
                console.warn('Failed to open external url:', err);
              });
            }}
            className="text-accent underline hover:text-accent-hover cursor-pointer"
          >
            {label}
          </a>
        );
      }
      return <span key={index} className="text-muted font-medium">{label}</span>;
    }

    return <span key={index}>{part}</span>;
  });
}

export default MarkdownView;
