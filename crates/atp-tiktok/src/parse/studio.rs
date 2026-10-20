//! Rows of TikTok Studio's content list (snake_case, counters as numbers
//! or strings).

use super::*;

/// One of your videos as Studio lists it; `own` is your handle when known
/// (links without one are redirected by TikTok).
pub fn studio_item(v: &Value, own: &str) -> Option<Post> {
  let id = v.first_str(&["item_id", "id"])?;
  let handle = v
    .first_str(&[
      "author.unique_id",
      "author.uniqueId",
      "author_unique_id",
      "user_name",
    ])
    .unwrap_or_else(|| own.to_owned());
  let text = v
    .first_str(&["desc", "title"])
    .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "));
  let metrics = Metrics {
    views: v.count("play_count"),
    likes: v.count("like_count"),
    comments: v.count("comment_count"),
    shares: v.count("share_count"),
    favorites: v.count("favorite_count"),
    ..Metrics::default()
  };
  Some(Post {
    url: Some(video_url(&handle, &id, false)),
    kind: "video".into(),
    text: text.filter(|t| !t.is_empty()),
    created_at: ["post_time", "create_time", "schedule_time"]
      .iter()
      .find_map(|k| v.i64(k).filter(|t| *t > 0))
      .and_then(from_secs),
    metrics,
    raw: Some(v.clone()),
    id,
    ..Post::default()
  })
}
