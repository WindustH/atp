//! The codes of the talk list's filters, as its page offers them: the
//! provinces of 切换城市 and the industries of 选择行业 (both kept here as
//! the page lists them), the hot schools of a province (read from the
//! province's page), dates.

use atp_core::{Collection, Error, Result, json};
use jiff::civil::Date;
use jiff::tz::{TimeZone, offset};
use regex::Regex;

use crate::api::{Api, MY};
use crate::refs;

/// Provinces and regions (`province`), as the page lists them.
const PROVINCES: &[(&str, &str)] = &[
  ("1", "北京"),
  ("6", "上海"),
  ("2", "天津"),
  ("27", "重庆"),
  ("13", "广东"),
  ("8", "江苏"),
  ("10", "山东"),
  ("3", "河北"),
  ("16", "河南"),
  ("9", "浙江"),
  ("28", "四川"),
  ("7", "安徽"),
  ("17", "湖北"),
  ("21", "辽宁"),
  ("5", "山西"),
  ("18", "湖南"),
  ("11", "福建"),
  ("22", "陕西"),
  ("12", "江西"),
  ("19", "黑龙江"),
  ("14", "广西"),
  ("23", "甘肃"),
  ("20", "吉林"),
  ("29", "云南"),
  ("30", "贵州"),
  ("4", "内蒙古"),
  ("26", "新疆"),
  ("15", "海南"),
  ("24", "宁夏"),
  ("25", "青海"),
  ("31", "西藏"),
  ("32", "香港"),
  ("33", "澳门"),
  ("34", "台湾"),
];

/// Industries (`hyid`), as the page lists them.
const INDUSTRIES: &[(&str, &str)] = &[
  ("25", "高校应届生专场招聘会"),
  ("18", "银行"),
  ("19", "证券、基金、保险及投资理财类"),
  ("10", "快速消费品"),
  ("24", "食品、农业、轻工类"),
  ("20", "制造、装备、电气及自动化设备"),
  ("8", "建筑、房地产、工程类"),
  ("5", "会计师事务所、财务公司"),
  ("4", "化工、生物、制药、医疗、医院"),
  ("15", "石油、钢铁、电力、能源、烟草"),
  ("14", "汽车行业"),
  ("11", "零售、贸易、家具"),
  ("12", "旅游、服装、餐饮、娱乐、酒店"),
  ("13", "媒体、公关、广告"),
  ("9", "交通、物流"),
  ("3", "互联网"),
  ("2", "电子、家电、电器"),
  ("1", "电信、移动及通信设备"),
  ("21", "咨询公司、律所、人力资源类"),
  ("6", "计算机软件"),
  ("7", "计算机硬件"),
  ("16", "芯片、半导体"),
  ("23", "高校、教育机构"),
  ("17", "研究所、国家机关、事业单位"),
  ("22", "其他"),
];

fn codes(list: &[(&str, &str)], kind: &str) -> Vec<Collection> {
  list
    .iter()
    .map(|(code, name)| {
      let mut item = Collection {
        id: (*code).to_owned(),
        kind: kind.to_owned(),
        name: (*name).to_owned(),
        ..Collection::default()
      };
      item.extra.insert("code".into(), json!(code));
      item
    })
    .collect()
}

pub fn provinces() -> Vec<Collection> {
  codes(PROVINCES, "province")
}

pub fn industries() -> Vec<Collection> {
  codes(INDUSTRIES, "industry")
}

/// A name without the words provinces and regions end with (省, 市,
/// 自治区 …), as the page writes them.
fn short(name: &str) -> &str {
  [
    "壮族自治区",
    "回族自治区",
    "维吾尔自治区",
    "自治区",
    "特别行政区",
    "省",
    "市",
  ]
  .iter()
  .find_map(|end| name.strip_suffix(end))
  .unwrap_or(name)
}

/// `--province`: a code as it is, or a name (浙江, 浙江省).
pub fn province(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  PROVINCES
    .iter()
    .find(|(_, name)| *name == short(arg))
    .map(|(code, _)| (*code).to_owned())
    .ok_or_else(|| {
      Error::input(format!(
        "Yingjiesheng knows no province {arg} (`atp yingjiesheng provinces` lists them)"
      ))
    })
}

/// `--industry`: a code as it is, or a name or part of one (互联网, 银行).
pub fn industry(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  INDUSTRIES
    .iter()
    .find(|(_, name)| *name == arg)
    .or_else(|| INDUSTRIES.iter().find(|(_, name)| name.contains(arg)))
    .filter(|_| !arg.is_empty())
    .map(|(code, _)| (*code).to_owned())
    .ok_or_else(|| {
      Error::input(format!(
        "Yingjiesheng knows no industry {arg} (`atp yingjiesheng industries` lists them)"
      ))
    })
}

/// `--date`: `2026-10-22`, `today` or `tomorrow` (in Beijing).
pub fn date(arg: &str) -> Result<String> {
  let today = || {
    jiff::Zoned::now()
      .with_time_zone(TimeZone::fixed(offset(8)))
      .date()
  };
  let date = match arg.trim() {
    "today" => Some(today()),
    "tomorrow" => today().tomorrow().ok(),
    text => text.parse::<Date>().ok(),
  };
  date.map(|d| d.to_string()).ok_or_else(|| {
    Error::input(format!(
      "--date takes a day as 2026-10-22, today or tomorrow (not {arg})"
    ))
  })
}

/// `--school` and `--city`: the site's number.
pub fn code(arg: &str, what: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  Err(Error::input(format!(
    "--{what} takes the site's number of the {what} ({what}_id of a listed talk{})",
    if what == "school" {
      ", or from `schools PROVINCE`"
    } else {
      ""
    }
  )))
}

/// The hot schools of a province, as its page lists them (热门学校).
pub async fn schools(api: &Api, province: &str) -> Result<Vec<Collection>> {
  let code = self::province(province)?;
  let html = api
    .page(&format!("{MY}/xuanjianghui_province_{code}.html"))
    .await?;
  let block = html
    .split_once("热门学校")
    .and_then(|(_, rest)| rest.split_once("</div>"))
    .map(|(block, _)| block)
    .unwrap_or_default();
  let link =
    Regex::new(r#"xuanjianghui_school_(\d+)\.html"[^>]*>([^<]+)<"#).expect("valid pattern");
  Ok(
    link
      .captures_iter(block)
      .map(|caps| {
        let (id, name) = (&caps[1], caps[2].trim());
        let mut school = Collection {
          id: id.to_owned(),
          kind: "school".into(),
          name: name.to_owned(),
          url: Some(refs::school_url(id)),
          ..Collection::default()
        };
        school.extra.insert("code".into(), json!(id));
        school
      })
      .collect(),
  )
}
