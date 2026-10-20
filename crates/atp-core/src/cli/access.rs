//! What a command does on the platform: only read, or change something.
//! The MCP `read` tool runs the commands that only read; the errors of a
//! command that changes something say whether the change may have
//! happened (`error.outcome`).

use clap::{ArgMatches, Subcommand};

use super::CommonCommand;
use crate::error::Error;

/// What a command does on the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
  /// It only reads: the MCP `read` tool runs it.
  Read,
  /// It may change something, keep files or spend a quota (likes, posts,
  /// downloads, logins ...): the MCP `run` tool runs it.
  Write,
}

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

/// Shared commands that change something on the platform. The other
/// writes (`login`, `logout`, `download`, `listen`) keep things on this
/// machine or wait for events.
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

/// What the command at `path` does: a shared command as listed here, a
/// platform-only one as the platform declares (`reads`,
/// [`Platform::EXTRA_READS`](crate::Platform::EXTRA_READS)).
pub fn access(path: &[&str], reads: &[&str]) -> Access {
  let read = match path.first() {
    Some(name) if CommonCommand::has_subcommand(name) => SHARED_READS.contains(name),
    Some(_) => reads.contains(&path.join(" ").as_str()),
    None => false,
  };
  if read { Access::Read } else { Access::Write }
}

/// Whether the command at `path` changes something on the platform: a
/// shared command that does, or a platform-only one not declared a read.
pub(crate) fn changes(path: &[&str], reads: &[&str]) -> bool {
  match path.first() {
    Some(name) if CommonCommand::has_subcommand(name) => SHARED_CHANGES.contains(name),
    Some(_) => access(path, reads) == Access::Write,
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

/// Debug builds: every declared read names a platform-only command of
/// `cmd` by its path, so a typo or a renamed command shows at once.
pub(crate) fn check_reads(cmd: &clap::Command, reads: &[&str]) {
  if !cfg!(debug_assertions) {
    return;
  }
  for read in reads {
    let words: Vec<&str> = read.split(' ').collect();
    let mut at = Some(cmd);
    for word in &words {
      at = at.and_then(|c| c.get_subcommands().find(|s| s.get_name() == *word));
    }
    assert!(
      at.is_some() && !CommonCommand::has_subcommand(words[0]),
      "{}: EXTRA_READS names `{read}`, which is no platform-only command",
      cmd.get_name()
    );
  }
}
