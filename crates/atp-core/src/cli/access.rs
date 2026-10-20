//! What a command does on the platform: only read, or change something.
//! The MCP `read` tool runs the commands that only read; the errors of a
//! command that changes something say whether the change may have
//! happened (`error.outcome`).

use clap::{ArgMatches, Subcommand};

use super::CommonCommand;
use crate::error::Error;

/// Shared commands that only read.
pub(crate) const SHARED_READS: &[&str] = &[
  "search",
  "hot",
  "feed",
  "read",
  "comments",
  "replies",
  "likers",
  "reposts",
  "insights",
  "user",
  "user-posts",
  "followers",
  "following",
  "blocked",
  "follow-groups",
  "follow-group",
  "collections",
  "favorites",
  "likes",
  "history",
  "watch-later",
  "notifications",
  "unread",
  "chats",
  "messages",
  "members",
  "whoami",
  "status",
];

/// Shared commands that change something on the platform. The others
/// that do not only read (`login`, `logout`, `download`, `listen`) keep
/// things on this machine or wait for events.
const SHARED_CHANGES: &[&str] = &[
  "like",
  "unlike",
  "favorite",
  "unfavorite",
  "comment",
  "delete-comment",
  "follow",
  "unfollow",
  "post",
  "delete",
  "edit",
  "send",
  "react",
  "remove-follower",
  "block",
  "unblock",
  "follow-group-create",
  "follow-group-rename",
  "follow-group-delete",
  "follow-group-add",
  "follow-group-remove",
  "folder-create",
  "folder-edit",
  "folder-delete",
  "folder-add",
  "folder-remove",
  "folder-move",
  "history-delete",
  "history-clear",
  "later-add",
  "later-remove",
  "later-clear",
];

/// The canonical names of the commands `matches` leads through, aliases
/// resolved: `["search"]`, `["playlist", "create"]`.
pub(crate) fn path(matches: &ArgMatches) -> Vec<&str> {
  let mut path = Vec::new();
  let mut at = matches;
  while let Some((name, sub)) = at.subcommand() {
    path.push(name);
    at = sub;
  }
  path
}

/// Whether the command at `path` changes something on the platform: a
/// shared command that does, or any platform-only command.
pub(crate) fn changes(path: &[&str]) -> bool {
  match path.first() {
    Some(name) if CommonCommand::has_subcommand(name) => SHARED_CHANGES.contains(name),
    Some(_) => true,
    None => false,
  }
}

/// The error as a command reports it: with what became of the change
/// when the command `changes` something, without an outcome otherwise.
pub(crate) fn settle(error: Error, changes: bool) -> Error {
  if changes {
    error.of_change()
  } else {
    error.of_read()
  }
}
