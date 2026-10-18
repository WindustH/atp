//! Discuz BBCode (`message_bbcode`) as plain text: formatting dropped,
//! links kept (`text (url)`), quotes as `> ` lines, lists as `- ` items,
//! pictures collected apart, attachments marked, and the part a `[hide]`
//! keeps from guests noted.
//!
//! Only the tags Discuz knows are read, and only when they open and close
//! in pairs (or stand alone, as `[*]` and `[hr]` do): `nums[i]` in a
//! 面经 stays as written.

/// The text of a post and what it holds besides.
#[derive(Debug, Default)]
pub struct Body {
  pub text: String,
  /// Links of `[img]` pictures.
  pub images: Vec<String>,
  /// A `[hide]` part keeps something from this reader.
  pub hidden: bool,
  /// The points (积分) `[hide=N]` asks for.
  pub credits: Option<u64>,
}

/// Tags that open and close.
const PAIRED: &[&str] = &[
  "b",
  "i",
  "u",
  "s",
  "strike",
  "sup",
  "sub",
  "size",
  "color",
  "font",
  "backcolor",
  "align",
  "p",
  "float",
  "indent",
  "left",
  "center",
  "right",
  "url",
  "email",
  "img",
  "attach",
  "attachimg",
  "media",
  "audio",
  "flash",
  "video",
  "code",
  "quote",
  "hide",
  "list",
  "table",
  "tr",
  "td",
  "free",
];
/// Tags that stand alone.
const SINGLE: &[&str] = &["*", "hr"];

#[derive(Debug)]
enum Token<'a> {
  Text(&'a str),
  Open {
    name: String,
    arg: Option<String>,
    raw: &'a str,
  },
  Close {
    name: String,
    raw: &'a str,
  },
}

#[derive(Debug)]
enum Node {
  Text(String),
  Tag {
    name: String,
    arg: Option<String>,
    children: Vec<Node>,
  },
}

pub fn parse(bbcode: &str) -> Body {
  let source = bbcode.replace("\r\n", "\n");
  let tokens = tokenize(&source);
  let mut at = 0;
  let nodes = tree(&tokens, &mut at, &mut Vec::new());
  let mut body = Body::default();
  let mut text = String::new();
  render(&nodes, &mut body, &mut text);
  body.text = tidy(&text);
  body
}

/// The tag a `[...]` at the start of `rest` is, with its length.
fn tag(rest: &str) -> Option<(Token<'_>, usize)> {
  let end = rest[1..].find([']', '[', '\n'])? + 1;
  if rest.as_bytes()[end] != b']' || end > 600 {
    return None;
  }
  let raw = &rest[..=end];
  let inner = &rest[1..end];
  if let Some(name) = inner.strip_prefix('/') {
    let name = name.trim().to_ascii_lowercase();
    return PAIRED
      .contains(&name.as_str())
      .then_some((Token::Close { name, raw }, raw.len()));
  }
  let (name, arg) = match inner.split_once('=') {
    Some((name, arg)) => (name, Some(arg.trim().trim_matches(['"', '\'']).to_owned())),
    None => (inner, None),
  };
  let name = name.trim().to_ascii_lowercase();
  let known = PAIRED.contains(&name.as_str()) || SINGLE.contains(&name.as_str());
  known.then_some((Token::Open { name, arg, raw }, raw.len()))
}

fn tokenize(source: &str) -> Vec<Token<'_>> {
  let mut tokens = Vec::new();
  let mut start = 0;
  let mut at = 0;
  while let Some(offset) = source[at..].find('[') {
    let open = at + offset;
    let Some((token, len)) = tag(&source[open..]) else {
      at = open + 1;
      continue;
    };
    if open > start {
      tokens.push(Token::Text(&source[start..open]));
    }
    let code = matches!(&token, Token::Open { name, .. } if name == "code");
    tokens.push(token);
    at = open + len;
    start = at;
    // What a `[code]` holds is code, not tags.
    if code && let Some(end) = find_close(&source[at..], "code") {
      tokens.push(Token::Text(&source[at..at + end]));
      at += end;
      start = at;
    }
  }
  if start < source.len() {
    tokens.push(Token::Text(&source[start..]));
  }
  tokens
}

/// Where `[/name]` starts in `text`, in any case.
fn find_close(text: &str, name: &str) -> Option<usize> {
  let close = format!("[/{name}]");
  text.to_ascii_lowercase().find(&close)
}

/// Nodes up to the close of a tag in `open` (left for that tag to take).
/// A tag that never closes is text; a close without its open is text.
fn tree(tokens: &[Token], at: &mut usize, open: &mut Vec<String>) -> Vec<Node> {
  let mut nodes = Vec::new();
  while let Some(token) = tokens.get(*at) {
    match token {
      Token::Text(text) => nodes.push(Node::Text((*text).to_owned())),
      Token::Close { name, raw } => {
        if open.contains(name) {
          return nodes;
        }
        nodes.push(Node::Text((*raw).to_owned()));
      }
      Token::Open { name, arg, .. } if SINGLE.contains(&name.as_str()) => nodes.push(Node::Tag {
        name: name.clone(),
        arg: arg.clone(),
        children: Vec::new(),
      }),
      Token::Open { name, arg, raw } => {
        *at += 1;
        open.push(name.clone());
        let children = tree(tokens, at, open);
        open.pop();
        let closed = matches!(tokens.get(*at), Some(Token::Close { name: n, .. }) if n == name);
        if closed {
          nodes.push(Node::Tag {
            name: name.clone(),
            arg: arg.clone(),
            children,
          });
        } else {
          // Never closed: the tag is text, its "children" follow it.
          nodes.push(Node::Text((*raw).to_owned()));
          nodes.extend(children);
          continue;
        }
      }
    }
    *at += 1;
  }
  nodes
}

fn plain(nodes: &[Node], body: &mut Body) -> String {
  let mut text = String::new();
  render(nodes, body, &mut text);
  text
}

fn render(nodes: &[Node], body: &mut Body, out: &mut String) {
  for node in nodes {
    match node {
      Node::Text(text) => out.push_str(text),
      Node::Tag {
        name,
        arg,
        children,
      } => tag_text(name, arg.as_deref(), children, body, out),
    }
  }
}

fn tag_text(name: &str, arg: Option<&str>, children: &[Node], body: &mut Body, out: &mut String) {
  match name {
    "url" | "email" => {
      let text = plain(children, body);
      out.push_str(&link(text.trim(), arg));
    }
    "img" => {
      let url = plain(children, body).trim().to_owned();
      if !url.is_empty() {
        body.images.push(url);
      }
    }
    "attach" | "attachimg" => out.push_str("[attachment]"),
    "media" | "audio" | "flash" | "video" => out.push_str(plain(children, body).trim()),
    "code" => {
      out.push('\n');
      out.push_str(plain(children, body).trim_matches('\n'));
      out.push('\n');
    }
    "quote" => {
      let text = plain(children, body);
      out.push('\n');
      for line in text.trim().lines().map(str::trim) {
        out.push_str(if line.is_empty() { ">" } else { "> " });
        out.push_str(line);
        out.push('\n');
      }
    }
    "hide" => {
      body.hidden = true;
      body.credits = arg.and_then(|a| a.split(',').next()?.trim().parse().ok());
      render(children, body, out);
    }
    "list" => list(arg, children, body, out),
    "*" => out.push_str("\n- "),
    "hr" => out.push_str("\n---\n"),
    "tr" => {
      render(children, body, out);
      out.push('\n');
    }
    "td" => {
      out.push_str(plain(children, body).trim());
      out.push('\t');
    }
    _ => render(children, body, out),
  }
}

/// A link's text with its target, unless they are the same; links to a
/// post (`goto=findpost`, quoted headers) keep only their text.
fn link(text: &str, target: Option<&str>) -> String {
  match target {
    Some(target) if !target.is_empty() && text.is_empty() => target.to_owned(),
    Some(target) if !target.is_empty() && text != target && !target.contains("goto=findpost") => {
      format!("{text} ({target})")
    }
    _ => text.to_owned(),
  }
}

/// `[list]` items as `- ` lines, `[list=1]` as `1.`, `2.` ….
fn list(arg: Option<&str>, children: &[Node], body: &mut Body, out: &mut String) {
  let numbered = arg.is_some_and(|a| a == "1");
  // Text before the first `[*]`, then one entry per item.
  let mut lead = String::new();
  let mut items: Vec<String> = Vec::new();
  for child in children {
    match child {
      Node::Tag { name, .. } if name == "*" => items.push(String::new()),
      other => {
        let into = items.last_mut().unwrap_or(&mut lead);
        render(std::slice::from_ref(other), body, into);
      }
    }
  }
  out.push('\n');
  if !lead.trim().is_empty() {
    out.push_str(lead.trim());
    out.push('\n');
  }
  for (n, item) in items.iter().enumerate() {
    let mark = if numbered {
      format!("{}. ", n + 1)
    } else {
      "- ".to_owned()
    };
    out.push_str(&mark);
    out.push_str(item.trim());
    out.push('\n');
  }
}

/// Lines without trailing blanks, at most one empty line in a row.
fn tidy(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut blank = 0;
  for line in text.lines().map(str::trim_end) {
    if line.is_empty() {
      blank += 1;
      if blank > 1 {
        continue;
      }
    } else {
      blank = 0;
    }
    out.push_str(line);
    out.push('\n');
  }
  out.trim().to_owned()
}
