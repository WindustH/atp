//! Insights of a business or creator account through the Graph API: an
//! Instagram-login token (`IG…`, graph.instagram.com) or a Facebook token of
//! the Page the account is linked to (graph.facebook.com). Asked one metric
//! at a time, since Meta retires names and one unknown name fails a request.

mod account;
mod post;

use atp_core::{Insights, Result};

use crate::api::Api;

const HINT: &str = "set INSTAGRAM_ACCESS_TOKEN (an Instagram-login token, or a Facebook token of the \
   linked Page with instagram_basic and instagram_manage_insights) or run \
   `atp instagram login --token <token>`; INSTAGRAM_USER_ID picks the account";

/// The account's metrics over the last `days` (at most 30).
pub async fn account(api: &Api, days: u32) -> Result<Insights> {
  let (graph, account) = api.graph_account("Instagram insights", HINT).await?;
  account::insights(&graph, &account, days).await
}

/// Lifetime metrics of one of the account's posts.
pub async fn post(api: &Api, post: &str) -> Result<Insights> {
  let (graph, account) = api.graph_account("Instagram insights", HINT).await?;
  post::insights(&graph, &account, post).await
}
