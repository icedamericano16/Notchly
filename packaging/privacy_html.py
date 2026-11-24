"""Renders docs/PRIVACY.md to docs/privacy.html (a standalone page for the Google OAuth
consent screen's privacy-policy URL, e.g. served by GitHub Pages from /docs).

Run after editing PRIVACY.md:  python packaging/privacy_html.py
Handles the Markdown the policy uses: headings, paragraphs, lists, tables, bold, italics,
inline code and links.
"""
import html
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "docs" / "PRIVACY.md"
DST = ROOT / "docs" / "privacy.html"


def inline(t: str) -> str:
    t = html.escape(t, quote=False)
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"(?<![*\w])\*([^*]+)\*(?!\*)", r"<em>\1</em>", t)
    t = re.sub(r"\[([^\]]+)\]\((https?://[^)\s]+)\)", r'<a href="\2">\1</a>', t)
    t = re.sub(r"&lt;(https?://[^&\s]+)&gt;", r'<a href="\1">\1</a>', t)
    return t


def render(md: str) -> str:
    out, para, items, table = [], [], [], []

    def flush():
        if para:
            out.append("<p>" + inline(" ".join(para)) + "</p>")
            para.clear()
        if items:
            out.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
            items.clear()
        if table:
            rows = [r for r in table if not re.match(r"^\|[\s|:-]+\|$", r)]
            cells = [[c.strip() for c in r.strip("|").split("|")] for r in rows]
            head = "".join(f"<th>{inline(c)}</th>" for c in cells[0])
            body = "".join(
                "<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in cells[1:]
            )
            out.append(f"<table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table>")
            table.clear()

    for line in md.splitlines():
        s = line.rstrip()
        if not s.strip():
            flush()
            continue
        m = re.match(r"^(#{1,3}) (.*)", s)
        if m:
            flush()
            n = len(m.group(1))
            out.append(f"<h{n}>{inline(m.group(2))}</h{n}>")
        elif s.startswith("|"):
            if para or items:
                flush()
            table.append(s)
        elif re.match(r"^\s*[-*] ", s):
            if para or table:
                flush()
            items.append(re.sub(r"^\s*[-*] ", "", s))
        elif items and s.startswith("  "):
            items[-1] += " " + s.strip()
        else:
            if table:
                flush()
            para.append(s.strip())
    flush()
    return "\n".join(out)


PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Notchly privacy policy</title>
<style>
  :root {{ --bg: #ffffff; --ink: #1d1d1f; --mist: #5b5b63; --line: #e3e3e8; --accent: #2d5bff; --code: #f3f3f6; }}
  @media (prefers-color-scheme: dark) {{
    :root {{ --bg: #0f1014; --ink: #ececf2; --mist: #a3a3ad; --line: #2a2b31; --accent: #7aa2ff; --code: #1b1c22; }}
  }}
  body {{ margin: 0; background: var(--bg); color: var(--ink);
         font: 16px/1.6 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif; }}
  main {{ max-width: 760px; margin: 0 auto; padding: 48px 16px 64px; }}
  h1 {{ font-size: 2rem; margin: 0 0 8px; }}
  h2 {{ font-size: 1.25rem; margin: 36px 0 8px; }}
  p, li {{ color: var(--ink); }}
  em {{ color: var(--mist); }}
  a {{ color: var(--accent); }}
  code {{ background: var(--code); padding: 1px 5px; border-radius: 5px; font-size: .9em; word-break: break-word; }}
  table {{ width: 100%; border-collapse: collapse; margin: 12px 0; font-size: .95rem; display: block; overflow-x: auto; }}
  th, td {{ text-align: left; vertical-align: top; padding: 8px 10px; border-bottom: 1px solid var(--line); }}
  th {{ font-weight: 600; }}
  footer {{ margin-top: 48px; color: var(--mist); font-size: .9rem; }}
</style>
</head>
<body>
<main>
{body}
<footer>Generated from docs/PRIVACY.md.</footer>
</main>
</body>
</html>
"""

if __name__ == "__main__":
    DST.write_text(PAGE.format(body=render(SRC.read_text(encoding="utf-8"))), encoding="utf-8")
    print(f"wrote {DST}")
