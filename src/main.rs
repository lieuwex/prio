mod util;

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hasher;
use std::path::Path;
use std::str::FromStr;
use std::time::SystemTime;
use std::{fs, path::PathBuf};

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
    #[serde(default)]
    read: bool,

    comment: Option<String>,

    #[serde(default)]
    tags: HashSet<String>,

    #[serde(flatten)]
    rest: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct File {
    path: PathBuf,
    hash: String,
    info: FileInfo,

    ordering: i64,
    checked: i64,
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
                    serde_yaml::from_str(&yaml).unwrap()
                } else {
                    Default::default()
                }
            };

            Some(File {
                path,
                hash: format!("{:x}", hash),
                info,
                ordering: 0,
                checked: 0,
            })
        })
        .collect()
}

fn map_row(row: &Row) -> File {
    let yaml: Option<String> = row.get_unwrap("info_yaml");
    let info: FileInfo = yaml
        .map(|yaml| serde_yaml::from_str(&yaml).unwrap())
        .unwrap_or_default();

    let path: String = row.get_unwrap("path");

    File {
        path: PathBuf::from(path),
        hash: row.get_unwrap("hash"),
        info,
        ordering: row.get_unwrap("ordering"),
        checked: row.get_unwrap("checked"),
    }
}

enum Ordering {
    None,
    Ordering,
    Checked,
}
fn get_db_files(conn: &mut Connection, ordering: Ordering) -> Vec<File> {
    let query = match ordering {
        Ordering::None => "SELECT * FROM entries",
        Ordering::Ordering => "SELECT * FROM entries ORDER BY ordering DESC",
        Ordering::Checked => "SELECT * FROM entries ORDER BY checked ASC",
    };

    conn.prepare(query)
        .unwrap()
        .query_map(params![], |r| Ok(map_row(r)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
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

    for f in fs_files {
        let ts = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs();
        let info_yaml = serde_yaml::to_string(&f.info).unwrap();

        let path = path_str(&f.path);

        tx.execute(
            r#"
            INSERT INTO entries
                (path, hash, info_yaml, added_at, updated_at)
            VALUES
                (?1, ?2, ?3, ?4, ?4)
            ON CONFLICT DO UPDATE SET
                hash = ?2,
                info_yaml = ?3,
                updated_at = ?4
        "#,
            params![path, f.hash, info_yaml, ts],
        )
        .unwrap();
    }

    tx.commit().unwrap();
}

fn competition(conn: &mut Connection, winner: &Path, loser: &Path) {
    assert!(winner != loser);

    let tx = conn.transaction().unwrap();

    tx.execute(
        "UPDATE entries SET ordering = ordering+1, checked = checked+1 WHERE path = ?1",
        params![path_str(&winner)],
    )
    .unwrap();

    tx.execute(
        "UPDATE entries SET ordering = ordering-1, checked = checked+1 WHERE path = ?1",
        params![path_str(&loser)],
    )
    .unwrap();

    tx.commit().unwrap();
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
    let mut conn = Connection::open("./db.db").unwrap();
    /*
    let mut rng = thread_rng();

    for f in get_files() {
        println!("{:?}", f);
    }

    set_db_files(&mut conn);
    */

    let items = get_db_files(&mut conn, Ordering::Checked);
    println!("{:?} {:?}", items[0], items[1]);

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .items(&items[0..2])
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap()
        .unwrap();

    let other = [1, 0][selection];
    competition(&mut conn, &items[selection].path, &items[other].path);
}
