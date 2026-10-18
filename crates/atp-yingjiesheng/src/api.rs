//! Yingjiesheng transport. The talks (宣讲会) live on the site's older
//! pages at `my.yingjiesheng.com`, server-rendered HTML in GBK: the list
//! (`/index.php/personal/xjhinfo.htm/?page=…&city=…&word=…`, the words in
//! GBK too), talk pages (`/xjh-006-299-612.html`) and province pages
//! (`/xuanjianghui_province_9.html`, with their hot schools). The mobile
//! site (`m.yingjiesheng.com/xuanjianghui/xjh_<id>`, UTF-8) renders the
//! same talks with their start and end times as data, where the older
//! pages show them as pictures.
//!
//! Aliyun's firewall guards parts of the site (www's pages, the job
//! search); a page that comes back as its check (`_waf_`, `aliyun_waf`)
//! is `verification_required` with the page. ATP never answers that
//! check, and does not follow redirects: a talk that is gone redirects to
//! www's 404 page, behind that firewall.

use atp_core::http::Resp;
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result};

/// The site.
pub const WWW: &str = "https://www.yingjiesheng.com";
/// The older pages, where the talks are.
pub const MY: &str = "https://my.yingjiesheng.com";
/// The mobile site.
pub const MOBILE: &str = "https://m.yingjiesheng.com";

const HTML: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// A page of the older site, decoded from GBK.
  pub async fn page(&self, url: &str) -> Result<String> {
    let resp = self
      .ctx
      .http
      .get(url)
      .header("accept", HTML)
      .header("referer", format!("{MY}/xuanjianghui.html"))
      .no_redirects()
      .send()
      .await?;
    let text = gbk(&resp.body);
    self.checked(&resp, text, url)
  }

  /// A page of the mobile site (UTF-8).
  pub async fn mobile(&self, url: &str) -> Result<String> {
    let resp = self
      .ctx
      .http
      .get(url)
      .header("accept", HTML)
      .header("referer", format!("{MOBILE}/xuanjianghui"))
      .no_redirects()
      .send()
      .await?;
    let text = resp.text();
    self.checked(&resp, text, url)
  }

  /// The page, or the error its answer stands for.
  fn checked(&self, resp: &Resp, text: String, url: &str) -> Result<String> {
    if text.contains("_waf_") || text.contains("aliyun_waf") {
      return Err(
        Error::verification(
          "Yingjiesheng's firewall (Aliyun WAF) wants a browser to pass its check first",
        )
        .with_url(url),
      );
    }
    match resp.status.as_u16() {
      404 => Err(Error::not_found(format!("Yingjiesheng has no page {url}"))),
      300..400 => Err(Error::not_found(format!(
        "Yingjiesheng has no page {url} (it sends to {})",
        resp.header("location").unwrap_or("another page")
      ))),
      _ if !resp.status.is_success() => Err(Error::upstream(format!(
        "Yingjiesheng answered {} for {url}: {}",
        resp.status,
        truncate(&one_line(&text), 120)
      ))),
      _ => Ok(text),
    }
  }
}

/// Text in GBK, the older pages' encoding (read as GB18030, its
/// superset).
pub fn gbk(bytes: &[u8]) -> String {
  let (text, _, _) = encoding_rs::GB18030.decode(bytes);
  text.into_owned()
}

/// Words for a query of the older pages: GBK bytes, percent-encoded.
pub fn gbk_query(text: &str) -> String {
  let (bytes, _, _) = encoding_rs::GBK.encode(text);
  bytes
    .iter()
    .map(|&b| match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
        (b as char).to_string()
      }
      _ => format!("%{b:02X}"),
    })
    .collect()
}
