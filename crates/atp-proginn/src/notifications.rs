//! Unread counts (`message/getUnreadCount`), as the site's header shows
//! them: `total`, `system` (系统消息), `work` (工作通知), `reply` (评论回复),
//! `at` (@我的), `community_other` (赞及其它) and `balance` (收支信息).

use std::collections::BTreeMap;

use atp_core::{Result, Value};

use crate::api::Api;

pub async fn unread(api: &Api) -> Result<BTreeMap<String, u64>> {
  let v = api.own("message/getUnreadCount").await?;
  let Value::Object(map) = v else {
    return Ok(BTreeMap::new());
  };
  Ok(
    map
      .iter()
      .filter_map(|(key, n)| {
        let n = n
          .as_u64()
          .or_else(|| n.as_str().and_then(|s| s.trim().parse().ok()))?;
        Some((key.clone(), n))
      })
      .collect(),
  )
}
