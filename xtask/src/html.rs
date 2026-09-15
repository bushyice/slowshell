// Source: DeepSeek V4.1 Flash
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::{Captures, Regex};

use pulldown_cmark::html::push_html;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use syntect::html::ClassedHTMLGenerator;
use syntect::html::{ClassStyle, css_for_theme_with_class_style};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use two_face::syntax::extra_newlines;
use two_face::theme::{EmbeddedThemeName, extra as extra_themes};

const WORD: &str = r"[A-Za-z_][A-Za-z0-9_]*";

static MIRROR_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?i)mirrors?\s+([A-Za-z0-9_./-]+\.rs)").unwrap());

static C_DEFINE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?m)^[ \t]*#[ \t]*define[ \t]+([A-Za-z_][A-Za-z0-9_]*)\b").unwrap()
});

static C_TYPEDEF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\btypedef\b").unwrap());

static FENCE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^```([\w-]*)\s*$").unwrap());

static CODE_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"(?s)<pre><code class="language-([\w-]+)">(.*?)</code></pre>"#).unwrap()
});

const NAMESPACE: &str = "readme";

static LOCAL_LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\]\(#([^)\s]+)\)").unwrap());

static HEADING_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.*?)\s*$").unwrap());

static SLUG_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w\- ]").unwrap());

static SLUG_DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-\s]+").unwrap());

#[derive(Debug)]
pub struct TocEntry {
  pub level: u32,
  pub id: String,
  pub name: String,
}

fn heading_level(level: &HeadingLevel) -> u32 {
  match level {
    HeadingLevel::H1 => 1,
    HeadingLevel::H2 => 2,
    HeadingLevel::H3 => 3,
    HeadingLevel::H4 => 4,
    HeadingLevel::H5 => 5,
    HeadingLevel::H6 => 6,
  }
}

fn collect_toc(events: &[Event<'_>]) -> Vec<TocEntry> {
  let mut toc: Vec<TocEntry> = Vec::new();
  let mut current: Option<(u32, Option<String>, String)> = None;

  for event in events {
    match event {
      Event::Start(Tag::Heading { level, id, .. }) => {
        current = Some((
          heading_level(level),
          id.as_ref().map(|id| id.to_string()),
          String::new(),
        ));
      }
      Event::Text(text) | Event::Code(text) => {
        if let Some((_, _, name)) = current.as_mut() {
          name.push_str(text);
        }
      }
      Event::End(TagEnd::Heading(_)) => {
        if let Some((level, Some(id), name)) = current.take() {
          toc.push(TocEntry { level, id, name });
        }
      }
      _ => {}
    }
  }

  toc
}

fn escape_html(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for character in text.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      '\'' => out.push_str("&#39;"),
      _ => out.push(character),
    }
  }
  out
}

fn html_unescape(text: &str) -> String {
  let decoded = text
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&quot;", "\"")
    .replace("&#39;", "'")
    .replace("&#x27;", "'");
  decoded.replace("&amp;", "&")
}

fn highlight_html(body: &str) -> String {
  CODE_BLOCK_RE
    .replace_all(body, |captures: &Captures| {
      let decoded = html_unescape(&captures[2]);
      let code = decoded.trim_end_matches('\n');
      match highlight_code(code, &captures[1]) {
        Some(highlighted) => highlighted,
        None => captures.get(0).unwrap().as_str().to_string(),
      }
    })
    .into_owned()
}

fn markdown_to_html(source: &str) -> (String, Vec<TocEntry>) {
  let mut options = Options::empty();
  options.insert(Options::ENABLE_TABLES);
  options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
  options.insert(Options::ENABLE_STRIKETHROUGH);
  options.insert(Options::ENABLE_FOOTNOTES);

  let events: Vec<Event<'_>> = Parser::new_ext(source, options).collect();
  let toc = collect_toc(&events);

  let mut body = String::new();
  push_html(&mut body, events.into_iter());

  (highlight_html(&body), toc)
}

fn toc_html(tokens: &[TocEntry]) -> String {
  let majors: Vec<&TocEntry> = tokens.iter().filter(|token| token.level == 1).collect();
  if majors.is_empty() {
    return String::new();
  }

  let items = majors
    .iter()
    .map(|token| {
      format!(
        "    <li><a href='#{}'>{}</a></li>",
        token.id,
        escape_html(&token.name)
      )
    })
    .collect::<Vec<_>>()
    .join("\n");

  format!("<nav class='majors' aria-label='Major sections'>\n  <ul>\n{items}\n  </ul>\n</nav>")
}

fn prefix_selectors(css: &str, prefix: &str) -> String {
  css
    .lines()
    .map(|line| {
      let Some(open) = line.find('{') else {
        return line.to_string();
      };
      let selector = line[..open].trim();
      if selector.is_empty() {
        return line.to_string();
      }
      let prefixed = selector
        .split(',')
        .map(|part| format!("{prefix}{}", part.trim()))
        .collect::<Vec<_>>()
        .join(", ");
      format!("{prefixed} {}", &line[open..])
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Recolour a stylesheet by swapping the Catppuccin hex values for the logo's.
fn tint(css: &str, map: &[(&str, &str)]) -> String {
  map
    .iter()
    .fold(css.to_string(), |css, (from, to)| css.replace(from, to))
}

const LATTE_TINT: &[(&str, &str)] = &[
  ("#eff1f5", "#eaf3f7"),
  ("#4c4f69", "#234151"),
  ("#8c8fa1", "#507382"),
  ("#7c7f93", "#507382"),
  ("#9ca0b0", "#547381"),
  ("#8839ef", "#2a5468"),
  ("#df8e1d", "#2f6883"),
  ("#fe640b", "#1f6b8c"),
  ("#40a02b", "#2e778e"),
  ("#179299", "#1f738f"),
  ("#04a5e5", "#1f7699"),
  ("#209fb5", "#1f738f"),
  ("#1e66f5", "#21607f"),
  ("#7287fd", "#3f6e8a"),
  ("#d20f39", "#4a6b7d"),
  ("#e64553", "#456a80"),
  ("#ea76cb", "#466f8a"),
  ("#dc8a78", "#4f7386"),
  ("#dd7878", "#4e7387"),
];

const MOCHA_TINT: &[(&str, &str)] = &[
  ("#1e1e2e", "#0e1c24"),
  ("#cdd6f4", "#dbeaf1"),
  ("#a6adc8", "#a9c6d3"),
  ("#7f849c", "#7fa6b6"),
  ("#8c8fa1", "#7fa6b6"),
  ("#9399b2", "#8fb3c2"),
  ("#cba6f7", "#a2d3e2"),
  ("#f9e2af", "#9fd0e0"),
  ("#fab387", "#8fc6dc"),
  ("#a6e3a1", "#7fd0c8"),
  ("#94e2d5", "#8fd6cf"),
  ("#89dceb", "#8fd6ea"),
  ("#74c7ec", "#8fd0ea"),
  ("#89b4fa", "#7cc0dd"),
  ("#b4befe", "#9cc9e0"),
  ("#f38ba8", "#6fa8c0"),
  ("#eba0ac", "#82b6cc"),
  ("#f5c2e7", "#b8dcea"),
  ("#f2cdcd", "#c2dfeb"),
  ("#f5e0dc", "#cfe6f0"),
];

fn syntax_css() -> String {
  let themes = extra_themes();
  let style = ClassStyle::SpacedPrefixed { prefix: "hl-" };

  let latte = tint(
    &css_for_theme_with_class_style(themes.get(EmbeddedThemeName::CatppuccinLatte), style)
      .unwrap_or_default(),
    LATTE_TINT,
  )
  .replace("Catppuccin Latte", "slowshell logo (light)");
  let mocha = tint(
    &prefix_selectors(
      &css_for_theme_with_class_style(themes.get(EmbeddedThemeName::CatppuccinMocha), style)
        .unwrap_or_default(),
      "html.dark ",
    ),
    MOCHA_TINT,
  )
  .replace("Catppuccin Mocha", "slowshell logo (dark)");

  format!("{latte}\n{mocha}\n{KDL_CSS}")
}

pub fn render_page(source: &str, title: &str) -> String {
  let (body, tokens) = markdown_to_html(source);

  format!(
    "<!DOCTYPE html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<title>{title}</title>
<link rel=\"icon\" type=\"image/png\" href=\"https://tangled.org/bushyice.com/slowshell/raw/main/assets/icon-shell.png\">
<link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">
<link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>
<link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?family=Montserrat:wght@400;700&family=Inconsolata:wght@400;700&family=Source+Code+Pro:wght@400&display=swap\">
<style>{css}</style>
<style>{syntax}</style>
</head>
<body>
<div class=\"container\">
{toc}
<main>
{body}
<footer>stuff here.<br>
<a href=\"https://tangled.org/bushyice.com/slowshell\">source</a></footer>
</main>
</div>
<script>{script}</script>
</body>
</html>
",
    title = escape_html(title),
    css = CSS,
    syntax = syntax_css(),
    toc = toc_html(&tokens),
    body = body,
    script = SCRIPT,
  )
}

const KDL_CSS: &str = r##"
.kdl-comment { color: #507382; font-style: italic; }
.kdl-string { color: #2e778e; }
.kdl-number { color: #1f6b8c; }
.kdl-keyword { color: #1f6b8c; }
.kdl-punct { color: #507382; }
.kdl-operator { color: #1f7699; }
html.dark .kdl-comment { color: #7fa6b6; }
html.dark .kdl-string { color: #7fd0c8; }
html.dark .kdl-number { color: #8fc6dc; }
html.dark .kdl-keyword { color: #8fc6dc; }
html.dark .kdl-punct { color: #8fb3c2; }
html.dark .kdl-operator { color: #8fd6ea; }
"##;

const CSS: &str = r##"
:root { /* slowshell logo: light */
  --bg: #f5fafc; --mantle: #eaf3f7; --crust: #ddedf3;
  --fg: #234151; --muted: #547685;
  --border: #bfd8e2; --surface: #d7e8ef;
  --pre-bg: #eaf3f7; --pre-fg: #234151;
  --link: #2c6e8a; --accent: #35697f; --accent-2: #1f4657;
}
html.dark { /* slowshell logo: dark */
  --bg: #0e1c24; --mantle: #0a151c; --crust: #061016;
  --fg: #dbeaf1; --muted: #8fb0be;
  --border: #27414d; --surface: #1b3039;
  --pre-bg: #081218; --pre-fg: #dbeaf1;
  --link: #a2d3e2; --accent: #7fbdd4; --accent-2: #a2d3e2;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
  margin: 2rem 0 5rem; background: var(--bg); color: var(--fg);
  font: 16px/1.5 Inconsolata, ui-monospace, SFMono-Regular, Menlo, monospace;
  transition: background-color .2s, color .2s;
}
::selection { background: var(--accent); color: var(--bg); }
a { color: var(--link); text-decoration: none; }
a:hover, a:focus { text-decoration: underline; }
h1, h2, h3, h4, h5, h6 {
  font-family: Montserrat, "Segoe UI", system-ui, sans-serif;
  margin: 0 0 .5rem -.1rem; line-height: 1.15;
  scroll-margin-top: 1rem;
}
h1 { font-size: 2.5rem; margin-bottom: 1rem; color: var(--accent-2); }
h2 { margin-top: 2.5rem; font-size: 1.5rem; margin-bottom: .75rem; color: var(--accent); }
h3, h4, h5, h6 { margin-top: 1.5rem; font-size: 1rem; text-transform: uppercase; letter-spacing: .03em; color: var(--fg); }
p, ul, ol, dl, table, pre, blockquote { margin-top: 0; margin-bottom: 1rem; }
ul, ol { padding-left: 1.5rem; }
li { margin: .15rem 0; }
li::marker { color: var(--accent); }
blockquote {
  margin-left: 0; margin-right: 0; padding: .5rem 1rem;
  border-left: .25rem solid var(--accent); color: var(--muted);
  background: var(--mantle); border-radius: 0 6px 6px 0;
}
blockquote p:last-child { margin-bottom: 0; }
hr { border: none; margin: 2rem 0; border-bottom: 1px solid var(--border); }
img { max-width: 100%; }
table { width: 100%; border: 1px solid var(--border); border-collapse: collapse; font-size: .9rem; }
th, td { padding: .3rem .55rem; border: 1px solid var(--border); text-align: left; vertical-align: top; }
th { font-family: Montserrat, sans-serif; background: var(--mantle); color: var(--accent); }
tr:nth-child(even) td { background: var(--mantle); }
pre, code { font-family: "Source Code Pro", ui-monospace, SFMono-Regular, Menlo, monospace; }
code { background: var(--surface); color: var(--accent); padding: .1rem .25rem; border-radius: 3px; font-size: .85em; }
pre { background: var(--pre-bg); color: var(--pre-fg); border: 1px solid var(--border); padding: .5rem 1rem; border-radius: 4px; overflow: auto; font-size: .8rem; }
pre code { background: none; color: inherit; padding: 0; font-size: inherit; }
/* Highlighted blocks: the wrapper owns the frame, the inner pre the scroll. */
.codehilite { background: var(--pre-bg); border: 1px solid var(--border); border-radius: 6px; overflow: hidden; margin-bottom: 1rem; }
.codehilite pre { margin: 0; border: 0; border-radius: 0; padding: .75rem 1rem; background: transparent; overflow-x: auto; }
.container { max-width: 44rem; margin: 0 auto; padding: 0 1rem; }

/* The README cover header: centered logo + wordmark + tagline. */
div[align="center"] { text-align: center; padding-bottom: 1rem; }
div[align="center"] img { display: block; width: 100px; height: auto; margin: 0 auto; cursor: pointer; border-radius: 12px; }
div[align="center"] h1 { color: var(--fg); margin: .5rem 0 0; }
div[align="center"] h1 code { background: none; color: inherit; padding: 0; font-family: inherit; font-size: inherit; }
div[align="center"] p { color: var(--muted); margin: .35rem 0 0; }

/* Only the major titles, anchored on the right. */
nav.majors { font-size: .85rem; }
nav.majors ul { list-style: none; margin: 0; padding: 0; }
nav.majors li { margin: .3rem 0; text-align: right; }
nav.majors a { color: var(--muted); }
nav.majors a:hover, nav.majors a.active { color: var(--accent); text-decoration: none; }
@media (min-width: 72rem) {
  nav.majors { position: fixed; top: 50%; right: 1.5rem; transform: translateY(-50%); }
}
@media (max-width: 71.99rem) {
  nav.majors { text-align: center; margin-bottom: 2.5rem; }
  nav.majors ul { display: flex; flex-wrap: wrap; gap: .35rem 1rem; justify-content: center; }
  nav.majors li { text-align: center; margin: 0; }
}

/* tldr.sh-style section anchors. */
.heading-anchor { color: var(--accent); margin-left: -1.1rem; margin-right: .25rem; visibility: hidden; }
h2:hover .heading-anchor, h3:hover .heading-anchor { visibility: visible; }
@media (max-width: 40rem) { .heading-anchor { display: none; } }

footer { color: var(--muted); font-size: .8rem; margin-top: 4rem; text-align: center; }
"##;

const SCRIPT: &str = r##"
const root = document.documentElement;
const logo =
  document.getElementById('logo') || document.querySelector('div[align="center"] img');

const media = window.matchMedia('(prefers-color-scheme: dark)');
const setTheme = dark => root.classList.toggle('dark', dark);
setTheme(media.matches);
media.addEventListener('change', event => setTheme(event.matches));
if (logo) {
  logo.title = 'Toggle theme';
  logo.addEventListener('click', () => root.classList.toggle('dark'));
}

for (const heading of document.querySelectorAll('h2[id], h3[id]')) {
  const anchor = document.createElement('a');
  anchor.className = 'heading-anchor';
  anchor.href = '#' + heading.id;
  anchor.setAttribute('aria-hidden', 'true');
  anchor.textContent = '\u00a7';
  heading.prepend(anchor);
}

const links = new Map();
for (const link of document.querySelectorAll('nav.majors a')) {
  const id = decodeURIComponent(link.hash.slice(1));
  if (document.getElementById(id)) links.set(id, link);
}
const observer = new IntersectionObserver(entries => {
  for (const entry of entries) {
    if (!entry.isIntersecting) continue;
    links.forEach(link => link.classList.remove('active'));
    links.get(entry.target.id)?.classList.add('active');
  }
}, { rootMargin: '0px 0px -80% 0px' });
links.forEach((_, id) => observer.observe(document.getElementById(id)));
"##;

fn slug(text: &str) -> String {
  let lowered = text.trim().to_lowercase();
  let stripped = SLUG_STRIP_RE.replace_all(&lowered, "");
  let dashed = SLUG_DASH_RE.replace_all(&stripped, "-");
  let trimmed = dashed.trim_matches('-');
  if trimmed.is_empty() {
    "section".to_string()
  } else {
    trimmed.to_string()
  }
}

fn namespace(text: &str) -> String {
  let mut lines: Vec<String> = Vec::new();
  let mut in_fence = false;

  for line in text.split('\n') {
    if FENCE_RE.is_match(line) {
      in_fence = !in_fence;
      lines.push(line.to_string());
      continue;
    }

    if in_fence {
      lines.push(line.to_string());
      continue;
    }

    let mut owned = LOCAL_LINK_RE
      .replace_all(line, |captures: &Captures| {
        format!("](#{NAMESPACE}-{})", captures[1].to_lowercase())
      })
      .into_owned();

    if let Some(heading) = HEADING_RE.captures(&owned) {
      let title = &heading[2];
      if !title.trim_end().ends_with('}') {
        owned = format!(
          "{} {} {{#{NAMESPACE}-{}}}",
          &heading[1],
          title.trim(),
          slug(title)
        );
      }
    }

    lines.push(owned);
  }

  lines.join("\n")
}

pub fn assemble(root: &Path) -> Result<String> {
  let readme = root.join("README.md");
  let source =
    fs::read_to_string(&readme).with_context(|| format!("reading {}", readme.display()))?;
  let body = resolve_includes(&source, root)?;
  Ok(format!("{}\n", namespace(body.trim())))
}
const CLASS_PREFIX: &str = "hl-";

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(extra_newlines);

fn syntax_for(lang: &str) -> Option<&'static SyntaxReference> {
  let lowered = lang.to_ascii_lowercase();
  let lookup = match lowered.as_str() {
    "rust" => "rs",
    "shell" => "sh",
    other => other,
  };

  let syntaxes = &*SYNTAXES;
  syntaxes
    .find_syntax_by_extension(lookup)
    .or_else(|| syntaxes.find_syntax_by_token(lookup))
}

pub fn highlight_code(code: &str, lang: &str) -> Option<String> {
  if lang.is_empty() {
    return None;
  }
  if lang.eq_ignore_ascii_case("kdl") {
    return Some(highlight_kdl(code));
  }
  if matches!(
    lang.to_ascii_lowercase().as_str(),
    "text" | "txt" | "plain" | "plaintext"
  ) {
    let mut out = String::from("<div class=\"codehilite\"><pre>");
    push_escaped(&mut out, code);
    out.push_str("</pre></div>");
    return Some(out);
  }

  let syntax = syntax_for(lang)?;
  let mut generator = ClassedHTMLGenerator::new_with_class_style(
    syntax,
    &SYNTAXES,
    ClassStyle::SpacedPrefixed {
      prefix: CLASS_PREFIX,
    },
  );

  let mut terminated = String::with_capacity(code.len() + 1);
  terminated.push_str(code);
  if !terminated.ends_with('\n') {
    terminated.push('\n');
  }

  for line in LinesWithEndings::from(&terminated) {
    if generator
      .parse_html_for_line_which_includes_newline(line)
      .is_err()
    {
      return None;
    }
  }

  Some(format!(
    "<div class=\"codehilite\"><pre>{}</pre></div>",
    generator.finalize()
  ))
}

fn push_escaped(out: &mut String, text: &str) {
  for character in text.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      _ => out.push(character),
    }
  }
}

fn push_span(out: &mut String, class: &str, text: &str) {
  out.push_str("<span class=\"");
  out.push_str(class);
  out.push_str("\">");
  push_escaped(out, text);
  out.push_str("</span>");
}

fn string_end(code: &str, start: usize) -> usize {
  let bytes = code.as_bytes();
  let mut i = start + 1;
  while i < bytes.len() {
    match bytes[i] {
      b'\\' => i += 2,
      b'"' => return i + 1,
      _ => i += 1,
    }
  }
  bytes.len()
}

fn highlight_kdl(code: &str) -> String {
  let bytes = code.as_bytes();
  let mut out = String::from("<div class=\"codehilite\"><pre>");
  let mut i = 0;

  while i < code.len() {
    let byte = bytes[i];
    let rest = &code[i..];

    if let Some(stripped) = rest.strip_prefix("//") {
      let end = stripped.find('\n').map_or(code.len(), |p| i + 2 + p);
      push_span(&mut out, "kdl-comment", &code[i..end]);
      i = end;
    } else if let Some(stripped) = rest.strip_prefix("/*") {
      let end = stripped.find("*/").map_or(code.len(), |p| i + 2 + p + 2);
      push_span(&mut out, "kdl-comment", &code[i..end]);
      i = end;
    } else if byte == b'"' {
      let end = string_end(code, i);
      push_span(&mut out, "kdl-string", &code[i..end]);
      i = end;
    } else if let Some(keyword) = ["#true", "#false", "#null"].iter().find(|keyword| {
      rest.starts_with(**keyword)
        && rest[keyword.len()..]
          .chars()
          .next()
          .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    }) {
      let end = i + keyword.len();
      push_span(&mut out, "kdl-keyword", &code[i..end]);
      i = end;
    } else if byte.is_ascii_digit()
      || ((byte == b'+' || byte == b'-' || byte == b'.')
        && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
    {
      let start = i;
      i += 1;
      while i < bytes.len()
        && (bytes[i].is_ascii_digit()
          || matches!(bytes[i], b'.' | b'_' | b'e' | b'E' | b'+' | b'-'))
      {
        i += 1;
      }
      push_span(&mut out, "kdl-number", &code[start..i]);
    } else if byte.is_ascii_alphabetic() || byte == b'_' {
      let start = i;
      i += 1;
      while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'.' | b'-'))
      {
        i += 1;
      }

      push_escaped(&mut out, &code[start..i]);
    } else if matches!(byte, b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';') {
      push_span(&mut out, "kdl-punct", &code[i..i + 1]);
      i += 1;
    } else if byte == b'=' {
      push_span(&mut out, "kdl-operator", &code[i..i + 1]);
      i += 1;
    } else {
      let character = rest.chars().next().unwrap();
      let end = i + character.len_utf8();
      push_escaped(&mut out, &code[i..end]);
      i = end;
    }
  }

  out.push_str("</pre></div>");
  out
}

fn dedent_block(text: &str) -> String {
  let mut lines: Vec<&str> = text.split('\n').collect();
  while lines.first().is_some_and(|line| line.trim().is_empty()) {
    lines.remove(0);
  }
  while lines.last().is_some_and(|line| line.trim().is_empty()) {
    lines.pop();
  }
  if lines.is_empty() {
    return String::new();
  }

  let indent = lines
    .iter()
    .filter(|line| !line.trim().is_empty())
    .map(|line| line.len() - line.trim_start().len())
    .min()
    .unwrap_or(0);

  lines
    .iter()
    .map(|line| {
      if line.trim().is_empty() {
        String::new()
      } else {
        line.get(indent..).unwrap_or("").to_string()
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn scan_balanced_c(text: &str, mut i: usize) -> usize {
  let bytes = text.as_bytes();
  let mut depth: i32 = 0;
  let mut in_string = false;
  let mut in_char = false;

  while i < bytes.len() {
    let c = bytes[i];
    if in_string {
      if c == b'\\' {
        i += 2;
        continue;
      }
      if c == b'"' {
        in_string = false;
      }
    } else if in_char {
      if c == b'\\' {
        i += 2;
        continue;
      }
      if c == b'\'' {
        in_char = false;
      }
    } else if c == b'"' {
      in_string = true;
    } else if c == b'\'' {
      in_char = true;
    } else if c == b'{' || c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b'}' || c == b')' || c == b']' {
      depth -= 1;
    } else if c == b';' && depth <= 0 {
      return i + 1;
    }
    i += 1;
  }
  bytes.len()
}

pub fn extract_c(source: &str, name: &str) -> Option<String> {
  let word = Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();

  for captures in C_DEFINE.captures_iter(source) {
    if &captures[1] != name {
      continue;
    }
    let whole = captures.get(0).unwrap();
    let mut line_end = source[whole.start()..]
      .find('\n')
      .map(|p| p + whole.start());
    while let Some(end) = line_end {
      if source[..end].trim_end().ends_with('\\') {
        line_end = source[end + 1..].find('\n').map(|p| p + end + 1);
      } else {
        break;
      }
    }
    let end = line_end.unwrap_or(source.len());
    return Some(dedent_block(&source[whole.start()..end]));
  }

  for definition in C_TYPEDEF.find_iter(source) {
    let end = scan_balanced_c(source, definition.start());
    let statement = &source[definition.start()..end];
    let tail = match statement.rfind('}') {
      Some(brace) => &statement[brace + 1..],
      None => statement,
    };
    if word.is_match(tail) {
      return Some(dedent_block(statement));
    }
  }

  let function = Regex::new(&format!(
    r"\b{WORD}[\w \t\*]*\b{}\s*\(",
    regex::escape(name)
  ))
  .unwrap();
  if let Some(call) = function.find_iter(source).next() {
    let start = source[..call.start()].rfind('\n').map_or(0, |p| p + 1);
    let end = scan_balanced_c(source, start);
    return Some(dedent_block(&source[start..end]));
  }

  None
}

fn scan_rust_block(text: &str, mut i: usize) -> usize {
  let bytes = text.as_bytes();
  let mut depth: i32 = 0;
  while i < bytes.len() {
    match bytes[i] {
      b'{' => depth += 1,
      b'}' => {
        depth -= 1;
        if depth == 0 {
          return i + 1;
        }
      }
      _ => {}
    }
    i += 1;
  }
  bytes.len()
}

fn scan_rust_stmt(text: &str, mut i: usize) -> usize {
  let bytes = text.as_bytes();
  let mut depth: i32 = 0;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' | b'{' => depth += 1,
      b')' | b']' | b'}' => depth -= 1,
      b';' if depth <= 0 => return i + 1,
      _ => {}
    }
    i += 1;
  }
  bytes.len()
}

pub fn extract_rust(source: &str, name: &str) -> Option<String> {
  let item = Regex::new(&format!(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(struct|enum|union|trait|fn|const|static|type|mod)\s+{name}\b|^[ \t]*macro_rules!\s+{name}\b",
    name = regex::escape(name)
  ))
  .unwrap();

  let found = item.find(source)?;

  let mut start = found.start();
  loop {
    let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
    if line_start == 0 {
      break;
    }
    let previous_newline = line_start - 1;
    let previous_start = source[..previous_newline].rfind('\n').map_or(0, |p| p + 1);
    let previous = source[previous_start..previous_newline].trim();
    if previous.starts_with('#') || previous.starts_with("///") || previous.starts_with("//!") {
      start = previous_start;
    } else {
      break;
    }
  }

  let after = found.end();
  let brace = source[after..].find('{').map(|p| p + after);
  let semi = source[after..].find(';').map(|p| p + after);
  let end = match (brace, semi) {
    (Some(brace), Some(semi)) if brace > semi => scan_rust_stmt(source, after),
    (Some(brace), _) => scan_rust_block(source, brace),
    (None, _) => scan_rust_stmt(source, after),
  };

  Some(dedent_block(&source[start..end]))
}

fn mirror_of(path: &Path, text: &str, root: &Path) -> Option<PathBuf> {
  if path.extension().and_then(|ext| ext.to_str()) != Some("h") {
    return None;
  }
  let captures = MIRROR_RE.captures(text)?;
  let candidate = root.join(&captures[1]);
  candidate.is_file().then_some(candidate)
}

fn render_symbol(name: &str, label: &str, lang: &str, code: &str) -> String {
  format!(
    "<!-- docs:symbol {label}#{name} -->\n**`{name}`** · {lang} · `{label}`\n\n```{lang}\n{code}\n```\n"
  )
}

pub fn resolve_ref(root: &Path, reference: &str) -> Result<String> {
  let Some((rel, symbol_blob)) = reference.split_once('#') else {
    bail!("missing '#Symbol' in reference: {reference:?}");
  };

  let path = root.join(rel);
  if !path.is_file() {
    bail!("no such file for symbol include: {rel}");
  }

  let text = fs::read_to_string(&path)?;
  let symbols: Vec<&str> = symbol_blob
    .split(',')
    .map(str::trim)
    .filter(|symbol| !symbol.is_empty())
    .collect();
  if symbols.is_empty() {
    bail!("no symbols given in reference: {reference:?}");
  }

  let mut out: Vec<String> = Vec::new();
  for symbol in symbols {
    if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
      let Some(code) = extract_rust(&text, symbol) else {
        bail!("{symbol}: not found in {rel}");
      };
      out.push(render_symbol(symbol, rel, "rust", &code));
      continue;
    }

    let Some(code) = extract_c(&text, symbol) else {
      bail!("{symbol}: not found in {rel}");
    };
    out.push(render_symbol(symbol, rel, "c", &code));

    if let Some(mirror) = mirror_of(&path, &text, root) {
      let mirror_text = fs::read_to_string(&mirror)?;
      if let Some(mirror_code) = extract_rust(&mirror_text, symbol) {
        let mirror_rel = mirror
          .strip_prefix(root)
          .unwrap_or(&mirror)
          .to_string_lossy()
          .replace('\\', "/");
        out.push(render_symbol(symbol, &mirror_rel, "rust", &mirror_code));
      }
    }
  }

  Ok(out.join("\n"))
}

pub fn resolve_includes(text: &str, root: &Path) -> Result<String> {
  let lines: Vec<&str> = text.split('\n').collect();
  let mut out: Vec<String> = Vec::new();
  let mut i = 0;

  while i < lines.len() {
    let Some(captures) = FENCE_RE.captures(lines[i]) else {
      out.push(lines[i].to_string());
      i += 1;
      continue;
    };
    if !captures[1].eq_ignore_ascii_case("symbol") {
      out.push(lines[i].to_string());
      i += 1;
      continue;
    }

    i += 1;
    let mut refs: Vec<String> = Vec::new();
    while i < lines.len() && lines[i].trim() != "```" {
      if !lines[i].trim().is_empty() {
        refs.extend(lines[i].split_whitespace().map(str::to_string));
      }
      i += 1;
    }
    if i >= lines.len() {
      bail!("unterminated ```symbol block");
    }
    i += 1;

    if refs.is_empty() {
      bail!("empty ```symbol block");
    }
    for reference in &refs {
      out.push(
        resolve_ref(root, reference)?
          .trim_end_matches('\n')
          .to_string(),
      );
    }
    out.push(String::new());
  }

  Ok(out.join("\n"))
}
