//! WeRead commands beyond the shared set.

use std::path::PathBuf;

use atp_core::cli::PageArgs;
use atp_core::paging::slice;
use atp_core::{Data, Result, Value};
use jiff::Timestamp;

use crate::api::Api;
use crate::{books, chapters, discover, notes, reading, shelf};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Your highlights (划线) and thoughts (想法) in a book, in reading order;
  /// without BOOK, the books you annotated
  #[command(after_long_help = EX_NOTES)]
  Notes {
    /// Book: id, reader or detail link, or #N; omit it to list annotated books
    book: Option<String>,
    /// Write the notes as Markdown to this file instead of listing them
    #[arg(long, value_name = "FILE", requires = "book")]
    markdown: Option<PathBuf>,
    #[command(flatten)]
    page: PageArgs,
  },
  /// How far you are in a book and how long you have read it
  #[command(after_long_help = EX_PROGRESS)]
  Progress {
    /// Book: id, reader or detail link, or #N
    book: String,
  },
  /// Totals of your shelf and notes: books, finished, started, notes
  Stats,
  /// Your reading statistics for a week, month, year or all time (needs an API key)
  #[command(after_long_help = EX_READING)]
  Reading {
    /// The period: week, month, year or all
    #[arg(long, default_value = "month", value_parser = ["week", "month", "year", "all"])]
    period: String,
    /// A time inside the period wanted (2026-09-01, 30d …); the current one by default
    #[arg(long, value_parser = atp_core::text::parse_when, value_name = "WHEN")]
    at: Option<Timestamp>,
  },
  /// Books of a store category or ranking by its id, in order
  #[command(after_long_help = EX_CATEGORY)]
  Category {
    /// Category id (`100000` 精品小说, `200000` 历史 …) or ranking id (`rising` …)
    id: String,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Books like one, as WeRead suggests them (needs an API key)
  Related {
    /// Book: id, reader or detail link, or #N
    book: String,
    #[command(flatten)]
    page: PageArgs,
  },
  /// A book's table of contents: chapters with their level, words and price
  Chapters {
    /// Book: id, reader or detail link, or #N
    book: String,
  },
  /// WeRead's AI outline of a book, chapter by chapter
  #[command(after_long_help = EX_OUTLINE)]
  Outline {
    /// Book: id, reader or detail link, or #N
    book: String,
    /// Deepest level shown: 2 topics, 3 key points, 4 details
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u64).range(2..=4))]
    depth: u64,
  },
  /// Where a word occurs in a book, with the passage around each place
  #[command(after_long_help = EX_BOOK_SEARCH)]
  BookSearch {
    /// Book: id, reader or detail link, or #N
    book: String,
    /// The word or phrase to look for
    query: String,
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_NOTES: &str = "\
Examples:
  atp weread notes                             # books you annotated
  atp weread notes '#1'                        # highlights and thoughts
  atp weread notes 695233 --markdown 三体.md
  atp weread notes https://weread.qq.com/web/bookDetail/ce032b305a9bc1ce0b0dd2a -f json

More: atp guide weread notes";

const EX_PROGRESS: &str = "\
Examples:
  atp weread favorites                         # your shelf
  atp weread progress '#1'

More: atp guide weread progress";

const EX_READING: &str = "\
Examples:
  atp weread reading                           # this month
  atp weread reading --period year --at 2025-06-01
  atp weread reading --period week -f json

More: atp guide weread reading";

const EX_CATEGORY: &str = "\
Examples:
  atp weread category 100000 -n 50             # 精品小说
  atp weread category 700000                   # 计算机

More: atp guide weread category";

const EX_OUTLINE: &str = "\
Examples:
  atp weread outline 695233
  atp weread outline '#1' --depth 3 -f csv > outline.csv

More: atp guide weread outline";

const EX_BOOK_SEARCH: &str = "\
Examples:
  atp weread book-search 695233 叶文洁
  atp weread book-search '#1' 'dark forest' -n 100 -f jsonl

More: atp guide weread book-search";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  let ctx = &api.ctx;
  Ok(match command {
    Command::Notes {
      book,
      markdown,
      page,
    } => notes(api, book, markdown, page).await?,
    Command::Progress { book } => Data::Value(shelf::progress(api, &ctx.post_ref(&book)?).await?),
    Command::Stats => Data::Value(shelf::stats(api).await?),
    Command::Reading { period, at } => {
      let at = at.map(|t| t.as_second());
      Data::Value(reading::reading(api, &period, at).await?)
    }
    Command::Category { id, page } => Data::Posts(
      page
        .collect_dated(async |req| books::category_books(api, &id, &req).await)
        .await?,
    ),
    Command::Related { book, page } => {
      let book = ctx.post_ref(&book)?;
      Data::Posts(
        page
          .collect(async |req| discover::similar(api, &book, &req).await)
          .await?,
      )
    }
    Command::Chapters { book } => rows(chapters::chapters(api, &ctx.post_ref(&book)?).await?),
    Command::Outline { book, depth } => {
      rows(chapters::outline(api, &ctx.post_ref(&book)?, depth).await?)
    }
    Command::BookSearch { book, query, page } => {
      let book = ctx.post_ref(&book)?;
      Data::Posts(
        page
          .collect(async |req| chapters::search(api, &book, &query, &req).await)
          .await?,
      )
    }
  })
}

async fn notes(
  api: &Api,
  book: Option<String>,
  markdown: Option<PathBuf>,
  page: PageArgs,
) -> Result<Data> {
  let Some(book) = book else {
    return Ok(Data::Posts(
      page
        .collect_dated(async |req| notes::notebooks(api, &req).await)
        .await?,
    ));
  };
  let found = notes::notes(api, &api.ctx.post_ref(&book)?).await?;
  if let Some(path) = markdown {
    return notes::export(&found, &path);
  }
  let items = notes::posts(found);
  Ok(Data::Posts(
    page
      .collect_dated(async |req| Ok(slice(items.clone(), &req)))
      .await?,
  ))
}

fn rows(items: Vec<Value>) -> Data {
  Data::Value(Value::Array(items))
}
