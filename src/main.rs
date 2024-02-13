mod util;

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hasher;
use std::path::Path;
use std::str::FromStr;
use std::time::SystemTime;
use std::{fs, path::PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use dialoguer::console::Term;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use metrohash::MetroHash;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use util::path_str;
use walkdir::WalkDir;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FileInfo {
    comment: Option<String>,

    #[serde(default)]
    tags: HashSet<String>,

    #[serde(flatten)]
    rest: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug)]
pub struct EntryOrdering {
    left_path: String,
    right_path: String,
    vote: i64,
    at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct HashChange {
    hash: u64,
    at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct File {
    path: PathBuf,
    info: Option<FileInfo>,
    hash_changes: Vec<HashChange>,
    _hash: u64,
    score: i64,
}

impl File {
    pub fn last_hash(&self) -> Option<u64> {
        self.hash_changes.last().map(|h| h.hash)
    }
}

impl Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.path)
    }
}

fn get_files() -> Vec<File> {
    WalkDir::new("entries")
        .into_iter()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            if !entry.file_type().is_file() {
                return None;
            }

            let path = entry.into_path();

            let s = fs::read_to_string(&path).unwrap();

            let hash = {
                let mut hasher = MetroHash::new();
                hasher.write(s.as_bytes());
                hasher.write_u8(0xff);
                hasher.finish()
            };

            let info = {
                let mut lines = s.lines();
                if let Some("---") = lines.next() {
                    let yaml: String = lines.take_while(|l| *l != "---").collect();
                    Some(serde_yaml::from_str(&yaml).unwrap())
                } else {
                    None
                }
            };

            Some(File {
                path,
                info,
                hash_changes: vec![],
                _hash: hash,
                score: 0,
            })
        })
        .collect()
}

enum Ordering {
    None,
    Ordering,
    Checked,
}
fn get_db_files(conn: &mut Connection, ordering: Ordering) -> Vec<File> {
    let mut b = conn
        .prepare("SELECT path FROM entries WHERE deleted = FALSE")
        .unwrap();
    let entries = b
        .query_map(params![], |r: &Row<'_>| -> Result<String, _> { r.get(0) })
        .unwrap();

    let mut hash_stmt = conn
        .prepare("SELECT hash, at FROM hash_changes WHERE path = ?1 ORDER BY at ASC")
        .unwrap();

    let mut items: HashMap<String, File> = entries
        .map(|entry| {
            let path = entry.unwrap();

            let hash_changes: Result<Vec<_>, _> = hash_stmt
                .query_map(params![path], |r| {
                    Ok(HashChange {
                        hash: {
                            let s: String = r.get(0)?;
                            u64::from_str_radix(&s, 16).unwrap()
                        },
                        at: Utc.timestamp_opt(r.get(1)?, 0).unwrap(),
                    })
                })
                .unwrap()
                .collect();

            (
                path.clone(),
                File {
                    path: PathBuf::from(path),
                    info: None,
                    hash_changes: hash_changes.unwrap(),
                    _hash: 0,
                    score: 0,
                },
            )
        })
        .collect();

    let mut c = conn
        .prepare("SELECT left_path, right_path, vote, at FROM entry_ordering")
        .unwrap();
    let orderings = c
        .query_map(params![], |r| {
            Ok(EntryOrdering {
                left_path: r.get(0)?,
                right_path: r.get(1)?,
                vote: r.get(2)?,
                at: Utc.timestamp_opt(r.get(3)?, 0).unwrap(),
            })
        })
        .unwrap();

    for ordering in orderings {
        let ordering = ordering.unwrap();

        items.get_mut(&ordering.left_path).unwrap().score -= ordering.vote;
        items.get_mut(&ordering.right_path).unwrap().score += ordering.vote;
    }

    let mut res: Vec<_> = items.into_iter().map(|p| p.1).collect();
    res.sort_by_key(|i| i.score);
    res
}

fn set_db_files(conn: &mut Connection) {
    let fs_files = get_files();
    let db_files = get_db_files(conn, Ordering::None);

    let tx = conn.transaction().unwrap();

    for f in db_files {
        if fs_files.iter().any(|fsf| fsf.path == f.path) {
            continue;
        }

        let path = path_str(&f.path);

        tx.execute(
            r#"
            DELETE FROM entries
            WHERE path = ?1
        "#,
            params![path],
        )
        .unwrap();
    }

    let mut exists_stmt = tx
        .prepare("SELECT COUNT(*)>0 FROM entries WHERE path = ?1")
        .unwrap();
    let mut prev_hash_stmt = tx
        .prepare("SELECT hash FROM hash_changes WHERE path = ?1 ORDER BY at DESC LIMIT 1")
        .unwrap();

    for f in fs_files {
        let ts = Utc::now().timestamp();
        let path = path_str(&f.path);

        let exists = exists_stmt
            .query_row(params![path], |r| Ok(r.get::<_, i64>(0)? == 1))
            .unwrap();

        if !exists {
            tx.execute(
                r#"
                INSERT INTO entries
                    (path, deleted)
                VALUES
                    (?1, ?2)
                "#,
                params![path, false],
            )
            .unwrap();
        }

        let prev_hash: Result<Vec<u64>, _> = prev_hash_stmt
            .query_map(params![path], |r| {
                let s: String = r.get(0)?;
                Ok(u64::from_str_radix(&s, 16).unwrap())
            })
            .unwrap()
            .collect();
        let prev_hash = prev_hash.unwrap().into_iter().next();

        let hash_same = prev_hash.is_some_and(|h| f._hash == h);
        if !hash_same {
            tx.execute(
                r#"
                INSERT INTO hash_changes
                    (path, hash, at)
                VALUES
                    (?1, ?2, ?3)
                "#,
                params![path, format!("{:x}", f._hash), ts],
            )
            .unwrap();
        }
    }

    drop(exists_stmt);
    drop(prev_hash_stmt);
    tx.commit().unwrap();
}

fn competition(conn: &mut Connection, winner: &Path, loser: &Path) {
    assert!(winner != loser);

    conn.execute(
        "INSERT INTO entry_ordering VALUES (?1, ?2, ?3, ?4)",
        params![path_str(loser), path_str(winner), 1, Utc::now().timestamp()],
    )
    .unwrap();
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

fn main() {
    for f in get_files() {
        println!("{:?}", f);
    }

    let mut conn = Connection::open("./db.db").unwrap();

    set_db_files(&mut conn);

    //let mut rng = thread_rng();

    let items = get_db_files(&mut conn, Ordering::Checked);

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .items(&items[0..2])
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap()
        .unwrap();

    let other = [1, 0][selection];
    competition(&mut conn, &items[selection].path, &items[other].path);

    println!("{:?} {:?}", items[0], items[1]);

    let items = get_db_files(&mut conn, Ordering::Checked);
    for f in items {
        println!("{:?}", f);
    }

    return;
    /*

    for f in get_files() {
        println!("{:?}", f);
    }

    set_db_files(&mut conn);
    */
}
