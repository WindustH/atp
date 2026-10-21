//! The bodies of `/tiktok/web/project/post/v1/` as Studio's upload page
//! builds them: a video as its form does without the editor (post type 3,
//! "project Y without cloud edit"), photos as its photo editor does (post
//! type 4, `aweme_type` 150), each with the page's defaults for what ATP
//! does not ask (comments, duets, stitches, reuse and AI remixes allowed,
//! no geofencing, no anchors, no commercial disclosure).

use atp_bytedance::upload::{Image, Video};
use atp_core::{Value, ValueExt, json};

use super::Post;

/// `enter_post_page_from` of Studio's menu (`creator_center`).
const FROM_STUDIO: i64 = 2;
const POST_VIDEO: i64 = 3;
const POST_PHOTOS: i64 = 4;
const AWEME_PHOTOS: i64 = 150;
/// `aigc_label_type` of a post its author labels AI-generated.
const AI_LABEL: i64 = 1;
/// What the page sends for no commercial disclosure.
const NO_TCM: &str = r#"{"commerce_toggle_info":{}}"#;

/// A video's request.
pub fn video(post: &Post, video: &Video, duration_ms: u64) -> Value {
  let common = video_common(post, &video.vid);
  let ranges = if duration_ms > 0 {
    json!([{ "start_time_in_ms": 0, "end_time_in_ms": duration_ms }])
  } else {
    json!([])
  };
  let c = &post.caption;
  json!({
    "post_common_info": {
      "creation_id": post.creation_id,
      "enter_post_page_from": FROM_STUDIO,
      "post_type": POST_VIDEO,
    },
    "feature_common_info_list": [common],
    "single_post_req_list": [{
      "batch_index": 0,
      "video_id": video.vid,
      "is_long_video": i64::from(post.long),
      "single_post_feature_info": {
        "text": c.text,
        "text_extra": c.text_extra,
        "markup_text": c.markup,
        "music_info": { "origin_volume": "100" },
        "poster_delay": 0,
        "cloud_edit_video_height": video.meta.u64("Height").unwrap_or(0),
        "cloud_edit_video_width": video.meta.u64("Width").unwrap_or(0),
        "cloud_edit_is_use_video_canvas": false,
        "has_original_audio": 1,
        "is_upload_audio_track": false,
        "video_track_time_range_list": ranges,
      },
    }],
  })
}

/// A video's settings (`feature_common_info_list[0]`), in the form's order.
fn video_common(post: &Post, vid: &str) -> Value {
  let mut common = json!({});
  if let Some(when) = post.schedule {
    common["schedule_time"] = when.into();
  }
  extend(
    &mut common,
    json!({
      "geofencing_regions": [],
      "playlist_name": post.playlist.as_ref().map_or("", |(_, name)| name.as_str()),
      "playlist_id": post.playlist.as_ref().map_or("", |(id, _)| id.as_str()),
      "tcm_params": NO_TCM,
      "sound_exemption": 0,
      "anchors": [],
    }),
  );
  if post.ai {
    common["aigc_info"] = json!({ "aigc_label_type": AI_LABEL });
  }
  extend(
    &mut common,
    json!({
      "vedit_common_info": { "draft": "", "video_id": vid },
      "privacy_setting_info": privacy(post),
      "content_check_id": "",
    }),
  );
  common
}

/// A photo post's request; `ids` are the page's ids of the photos.
pub fn photos(post: &Post, photos: &[Image], ids: &[String]) -> Value {
  let images: Vec<Value> = photos
    .iter()
    .zip(ids)
    .map(|(p, id)| json!({ "id": id, "uri": p.uri, "width_px": p.width, "height_px": p.height }))
    .collect();
  let mut common = json!({
    "draft_id": "",
    "anchors": [],
    "aigc_info": { "aigc_label_type": if post.ai { AI_LABEL } else { 0 } },
  });
  if let Some(when) = post.schedule {
    common["schedule_time"] = when.into();
  }
  common["privacy_setting_info"] = privacy(post);
  if let Some((id, name)) = &post.playlist {
    common["playlist_id"] = id.as_str().into();
    common["playlist_name"] = name.as_str().into();
  }
  common["tcm_params"] = NO_TCM.into();
  let c = &post.caption;
  json!({
    "post_common_info": {
      "creation_id": post.creation_id,
      "enter_post_page_from": FROM_STUDIO,
      "post_type": POST_PHOTOS,
    },
    "single_post_req_list": [{
      "batch_index": 0,
      "aweme_type": AWEME_PHOTOS,
      "single_post_feature_info": {
        "text": c.text,
        "text_extra": c.text_extra,
        "markup_text": c.markup,
      },
      "image_post_content": {
        "title": post.title,
        "images": images,
        "cover": images.first().cloned().unwrap_or(Value::Null),
      },
    }],
    "feature_common_info_list": [common],
  })
}

/// Who may see and reuse the post. Videos over a minute get no duets,
/// stitches or reuse, as the page sets them.
fn privacy(post: &Post) -> Value {
  let reuse = i64::from(!post.long);
  json!({
    "visibility_type": post.visibility,
    "allow_duet": reuse,
    "allow_stitch": reuse,
    "allow_comment": 1,
    "allow_content_reuse": reuse,
    // The page's `allow_ai_remix` is 1 for yes, 2 for no.
    "allow_ai_remix": 1,
  })
}

/// Add `more`'s fields to `object`, in order.
fn extend(object: &mut Value, more: Value) {
  if let (Some(object), Value::Object(more)) = (object.as_object_mut(), more) {
    object.extend(more);
  }
}
