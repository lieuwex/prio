mod util;

use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use dialoguer::console::Term;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use rand::{thread_rng, Rng};
use skillratings::{
    glicko2::{glicko2, Glicko2Config, Glicko2Rating},
    Outcomes,
};
use sqlx::{query, Connection, SqliteConnection};
use tokio::fs;
use tokio::runtime::Builder;
use walkdir::WalkDir;

use util::path_str;

// TODO maak manier om files te moven en dat te volgen

const PATH: &'static str = "/home/lieuwe/entries";
const DB_PATH: &'static str = "/home/lieuwe/entries/.db.db";

async fn competition(conn: &mut SqliteConnection, winner: &Path, loser: &Path) -> Result<()> {
    assert!(winner != loser);

    let winner = path_str(winner);
    let loser = path_str(loser);
    let score = 1;
    let ts = Utc::now().timestamp();

    query!(
        "INSERT INTO entry_votes VALUES (?1, ?2, ?3, ?4)",
        winner,
        loser,
        score,
        ts
    )
    .execute(conn)
    .await?;
    Ok(())
}

fn take_n_random<T>(rng: &mut impl Rng, items: &mut Vec<T>, n: usize) -> Vec<T> {
    let mut res = Vec::with_capacity(n);

    for _ in 0..n {
        if items.len() == 0 {
            panic!("vec is empty, but more items are requested");
        }

        let i = rng.gen_range(0..items.len());
        res.push(items.remove(i));
    }

    res
}

fn take_n_most_interesting(items: &mut VecDeque<File>, n: usize) -> Vec<File> {
    items
        .make_contiguous()
        .sort_by_key(|f| f.rating.deviation as i64);

    let mut res = Vec::with_capacity(n);
    for _ in 0..n {
        res.push(
            items
                .pop_back()
                .expect("vec is empty, but more items are requested"),
        );
    }
    res
}

#[derive(Debug, Clone)]
pub struct FileContent {
    content: Vec<u8>,
    at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Vote {
    left_path: PathBuf,
    right_path: PathBuf,
    vote: i64,
    at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct File {
    path: PathBuf,
    deleted: bool,
    file_contents: Vec<FileContent>,
    rating: Glicko2Rating,
}

impl File {
    fn last_content(&self) -> &FileContent {
        self.file_contents
            .last()
            .expect("file_contents can't be empty")
    }
}

impl Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = &self
            .file_contents
            .last()
            .expect("file_contents can't be empty")
            .content;
        let s = std::str::from_utf8(&content).unwrap();
        let line = s.lines().nth(0).unwrap_or("");
        write!(f, "{} ({})", line, path_str(&self.path))
    }
}

// REVIEW
impl PartialEq for File {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}
impl Eq for File {}

// REVIEW
impl Hash for File {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state)
    }
}

async fn get_db_files(conn: &mut SqliteConnection, include_deleted: bool) -> Result<Vec<File>> {
    let items = query!(
        r#"
            SELECT path, deleted
            FROM entries
        "#
    )
    .map(|r| File {
        path: PathBuf::from(r.path),
        deleted: r.deleted,
        file_contents: vec![],
        rating: Glicko2Rating::new(),
    })
    .fetch_all(conn.borrow_mut())
    .await?;

    let mut m = HashMap::with_capacity(items.len());
    for mut item in items {
        let item_path = item.path.to_str().unwrap();
        let contents = query!(
            r#"
                SELECT content, at
                FROM file_contents
                WHERE path = ?1
                ORDER BY at ASC
            "#,
            item_path,
        )
        .map(|r| FileContent {
            content: r.content,
            at: Utc.timestamp(r.at, 0),
        })
        .fetch_all(conn.borrow_mut())
        .await?;

        item.file_contents = contents;

        m.insert(item.path.clone(), item);
    }

    let orderings = query!(
        r#"
            SELECT left_path, right_path, vote, at
            FROM entry_votes
        "#
    )
    .map(|r| Vote {
        left_path: PathBuf::from(r.left_path),
        right_path: PathBuf::from(r.right_path),
        vote: r.vote,
        at: Utc.timestamp(r.at, 0),
    })
    .fetch_all(conn.borrow_mut())
    .await?;

    for ordering in orderings {
        let left = m.get(&ordering.left_path).unwrap().rating;
        let right = m.get(&ordering.right_path).unwrap().rating;

        let outcome = match ordering.vote {
            0 => Outcomes::DRAW,
            ..=-1 => Outcomes::LOSS,
            1.. => Outcomes::WIN,
        };

        let (left, right) = glicko2(&left, &right, &outcome, &Glicko2Config::new());

        m.get_mut(&ordering.left_path).unwrap().rating = left;
        m.get_mut(&ordering.right_path).unwrap().rating = right;
    }

    let mut res: Vec<_> = m
        .into_iter()
        .map(|p| p.1)
        .filter(|f| !f.deleted || include_deleted)
        .collect();
    res.sort_by_key(|i| i.rating.rating as i64);
    Ok(res)
}

async fn update_files(conn: &mut SqliteConnection) -> Result<()> {
    let entries = WalkDir::new(PATH).into_iter().filter_map(|entry| {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            return None;
        } else if entry.file_name().to_string_lossy().starts_with('.') {
            return None;
        }

        Some(entry)
    });

    let db_files = get_db_files(conn, true).await?;
    let mut left: HashSet<&File> = db_files.iter().filter(|f| !f.deleted).collect();

    for entry in entries {
        let metadata = entry.metadata().unwrap();
        let modified: DateTime<Utc> = metadata.modified().unwrap().into();

        let full_path = entry.path().to_path_buf();
        let path = full_path.strip_prefix(PATH).unwrap();
        let path_str = path.to_str().unwrap();

        let db_file = db_files.iter().find(|f| f.path == path);

        match db_file {
            None => {
                query!(
                    r#"
                    INSERT INTO entries
                        (path, deleted)
                    VALUES
                        (?1, ?2)
                    "#,
                    path_str,
                    false
                )
                .execute(conn.borrow_mut())
                .await?;
            }
            Some(db_file) if db_file.deleted => {
                // TODO: make this a warning
                panic!("file already exists in database as deleted");
            }
            Some(db_file) => {
                left.remove(&db_file);

                let outdated = modified > db_file.last_content().at;
                if !outdated {
                    continue;
                }
            }
        }

        let bytes = fs::read(&full_path).await?;

        match db_file {
            Some(f) if f.last_content().content == bytes => continue,
            None | Some(_) => {
                let ts = Utc::now().timestamp();

                query!(
                    r#"
                    INSERT INTO file_contents
                        (path, content, at)
                    VALUES
                        (?1, ?2, ?3)
                    "#,
                    path_str,
                    bytes,
                    ts
                )
                .execute(conn.borrow_mut())
                .await?;
            }
        }
    }

    for db_file in left {
        let path = path_str(&db_file.path);
        query!("UPDATE entries SET deleted = 1 WHERE path = ?1", path)
            .execute(conn.borrow_mut())
            .await?;
    }

    Ok(())
}

fn main() -> Result<()> {
    Builder::new_current_thread().build()?.block_on(async {
        //let mut rng = thread_rng();
        let mut conn = SqliteConnection::connect(DB_PATH).await?;

        update_files(&mut conn).await?;

        let items = get_db_files(&mut conn, false).await?;
        let mut items = VecDeque::from(items);
        let items = take_n_most_interesting(&mut items, 2);

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .items(&items)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .unwrap()
            .unwrap();

        let other = [1, 0][selection];
        competition(&mut conn, &items[selection].path, &items[other].path).await?;

        let items = get_db_files(&mut conn, false).await?;
        for item in items.into_iter().rev() {
            println!(
                "{} (score: {}, deviation: {})",
                item, item.rating.rating as i64, item.rating.deviation as i64
            );
        }

        Ok(())
    })
}
