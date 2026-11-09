// Gemini's replies as reading text: a small subset of Markdown (paragraphs, lists, **bold**,
// `code`, fenced code blocks, and headings set as bold lines) turned into React elements. Never
// HTML: whatever the text holds is rendered as text.

import type { ReactNode } from "react";

type Block =
  | { kind: "paragraph"; text: string }
  | { kind: "heading"; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "code"; text: string };

const BULLET = /^\s*[-*•]\s+(.*)$/;
const NUMBERED = /^\s*\d+[.)]\s+(.*)$/;

export function parseBlocks(source: string): Block[] {
  const blocks: Block[] = [];
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length > 0) blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
    paragraph = [];
  };
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i] ?? "";
    if (line.trimStart().startsWith("```")) {
      flush();
      const code: string[] = [];
      i += 1;
      while (i < lines.length && !(lines[i] ?? "").trimStart().startsWith("```")) {
        code.push(lines[i] ?? "");
        i += 1;
      }
      blocks.push({ kind: "code", text: code.join("\n") });
      continue;
    }
    const heading = /^\s*#{1,6}\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      blocks.push({ kind: "heading", text: heading[1] ?? "" });
      continue;
    }
    const bullet = BULLET.exec(line);
    const numbered = NUMBERED.exec(line);
    if (bullet || numbered) {
      flush();
      const ordered = !bullet;
      const last = blocks[blocks.length - 1];
      const item = (bullet ?? numbered)?.[1] ?? "";
      if (last?.kind === "list" && last.ordered === ordered) last.items.push(item);
      else blocks.push({ kind: "list", ordered, items: [item] });
      continue;
    }
    if (line.trim() === "") {
      flush();
      continue;
    }
    // A wrapped line continues the list item above it.
    const last = blocks[blocks.length - 1];
    if (paragraph.length === 0 && last?.kind === "list" && /^\s{2,}/.test(line)) {
      const at = last.items.length - 1;
      last.items[at] = `${last.items[at] ?? ""} ${line.trim()}`;
      continue;
    }
    paragraph.push(line.trim());
  }
  flush();
  return blocks;
}

/** `code` and **bold** inside a line. */
export function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*)/g;
  let last = 0;
  let key = 0;
  for (const match of text.matchAll(pattern)) {
    const at = match.index;
    if (at > last) out.push(text.slice(last, at));
    const token = match[0];
    if (token.startsWith("`")) out.push(<code key={key++}>{token.slice(1, -1)}</code>);
    else out.push(<strong key={key++}>{token.slice(2, -2)}</strong>);
    last = at + token.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Prose({ text, className }: { text: string; className?: string }) {
  return (
    <div className={className}>
      {parseBlocks(text).map((block, index) => {
        switch (block.kind) {
          case "heading":
            return (
              <p key={index}>
                <strong>{inline(block.text)}</strong>
              </p>
            );
          case "list": {
            const List = block.ordered ? "ol" : "ul";
            return (
              <List key={index}>
                {block.items.map((item, i) => (
                  <li key={i}>{inline(item)}</li>
                ))}
              </List>
            );
          }
          case "code":
            return (
              <pre key={index}>
                <code>{block.text}</code>
              </pre>
            );
          default:
            return <p key={index}>{inline(block.text)}</p>;
        }
      })}
    </div>
  );
}
