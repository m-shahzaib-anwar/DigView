use crate::model::*;
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, String>;
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn connect(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .map_err(|e| e.to_string())?;
    // This database is a rebuildable index, not the user's file contents.
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")
        .map_err(|e| e.to_string())?;
    Ok(conn)
}
pub fn init(path: &Path) -> Result<()> {
    let conn = connect(path)?;
    let had_totals: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='totals')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute_batch("
      PRAGMA journal_mode=WAL;
      CREATE TABLE IF NOT EXISTS roots(id INTEGER PRIMARY KEY,path TEXT UNIQUE NOT NULL,added_at INTEGER NOT NULL,last_scan INTEGER);
      CREATE TABLE IF NOT EXISTS files(
        id INTEGER PRIMARY KEY AUTOINCREMENT,root_id INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
        path TEXT UNIQUE NOT NULL,name TEXT NOT NULL,extension TEXT NOT NULL,category TEXT NOT NULL,
        size INTEGER NOT NULL,modified INTEGER NOT NULL,created INTEGER,source TEXT,screenshot INTEGER NOT NULL DEFAULT 0,
        hash TEXT,seen INTEGER NOT NULL DEFAULT 0,modified_token TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS files_category ON files(category,modified DESC,id DESC);
      CREATE INDEX IF NOT EXISTS files_size ON files(size DESC,id DESC);
      CREATE INDEX IF NOT EXISTS files_modified ON files(modified DESC,id DESC);
      CREATE INDEX IF NOT EXISTS files_source ON files(source);
      CREATE INDEX IF NOT EXISTS files_hash ON files(hash) WHERE hash IS NOT NULL;
      CREATE INDEX IF NOT EXISTS files_root ON files(root_id);
      CREATE INDEX IF NOT EXISTS files_name ON files(name COLLATE NOCASE,id);
      CREATE INDEX IF NOT EXISTS files_category_root_totals ON files(category,root_id,size);
      CREATE INDEX IF NOT EXISTS files_source_recent ON files(source,modified DESC,id DESC);
      CREATE INDEX IF NOT EXISTS files_root_recent ON files(root_id,modified DESC,id DESC);
      CREATE INDEX IF NOT EXISTS files_old_screenshots ON files(modified) WHERE screenshot=1;
      CREATE INDEX IF NOT EXISTS files_old_downloads ON files(modified) WHERE source IS NOT NULL OR lower(replace(path,char(92),'/')) LIKE '%/downloads/%';
      CREATE TABLE IF NOT EXISTS totals(scope TEXT PRIMARY KEY,file_count INTEGER NOT NULL,bytes INTEGER NOT NULL);
      INSERT OR IGNORE INTO totals VALUES('all',0,0);
      CREATE TRIGGER IF NOT EXISTS totals_ai AFTER INSERT ON files BEGIN
        INSERT INTO totals VALUES('all',1,new.size),('category:'||new.category,1,new.size),('root:'||new.root_id,1,new.size)
          ON CONFLICT(scope) DO UPDATE SET file_count=file_count+1,bytes=bytes+excluded.bytes;
        INSERT INTO totals SELECT 'source:'||new.source,1,new.size WHERE new.source IS NOT NULL
          ON CONFLICT(scope) DO UPDATE SET file_count=file_count+1,bytes=bytes+excluded.bytes;
      END;
      CREATE TRIGGER IF NOT EXISTS totals_ad AFTER DELETE ON files BEGIN
        UPDATE totals SET file_count=file_count-1,bytes=bytes-old.size WHERE scope IN ('all','category:'||old.category,'root:'||old.root_id,'source:'||old.source);
      END;
      CREATE TRIGGER IF NOT EXISTS totals_au AFTER UPDATE OF size,category,root_id,source ON files
        WHEN old.size!=new.size OR old.category!=new.category OR old.root_id!=new.root_id OR old.source IS NOT new.source BEGIN
        UPDATE totals SET file_count=file_count-1,bytes=bytes-old.size WHERE scope IN ('all','category:'||old.category,'root:'||old.root_id,'source:'||old.source);
        INSERT INTO totals VALUES('all',1,new.size),('category:'||new.category,1,new.size),('root:'||new.root_id,1,new.size)
          ON CONFLICT(scope) DO UPDATE SET file_count=file_count+1,bytes=bytes+excluded.bytes;
        INSERT INTO totals SELECT 'source:'||new.source,1,new.size WHERE new.source IS NOT NULL
          ON CONFLICT(scope) DO UPDATE SET file_count=file_count+1,bytes=bytes+excluded.bytes;
      END;
      CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(name,content='files',content_rowid='id',tokenize='unicode61');
      CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN INSERT INTO files_fts(rowid,name) VALUES(new.id,new.name); END;
      CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN INSERT INTO files_fts(files_fts,rowid,name) VALUES('delete',old.id,old.name); END;
      CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE OF name ON files WHEN old.name != new.name BEGIN
        INSERT INTO files_fts(files_fts,rowid,name) VALUES('delete',old.id,old.name);
        INSERT INTO files_fts(rowid,name) VALUES(new.id,new.name); END;
    ").map_err(|e| e.to_string())?;
    crate::discovery::init(&conn)?;
    crate::setup::init(&conn, had_totals)?;
    crate::visibility::init(&conn)?;
    crate::document_index::init(&conn)?;
    if !had_totals {
        conn.execute_batch("BEGIN;
          INSERT OR REPLACE INTO totals SELECT 'all',count(*),coalesce(sum(size),0) FROM files;
          INSERT OR REPLACE INTO totals SELECT 'category:'||category,count(*),sum(size) FROM files GROUP BY category;
          INSERT OR REPLACE INTO totals SELECT 'root:'||root_id,count(*),sum(size) FROM files GROUP BY root_id;
          INSERT OR REPLACE INTO totals SELECT 'source:'||source,count(*),sum(size) FROM files WHERE source IS NOT NULL GROUP BY source;
          COMMIT;").map_err(|e|e.to_string())?;
    }
    Ok(())
}
fn totals(conn: &Connection, scope: &str) -> Result<(i64, i64)> {
    if crate::visibility::has_hidden(conn)? {
        let (filter, value) = if scope == "all" {
            ("1=1", None)
        } else if let Some(v) = scope.strip_prefix("category:") {
            ("category=?", Some(v))
        } else if let Some(v) = scope.strip_prefix("source:") {
            ("source=?", Some(v))
        } else if let Some(v) = scope.strip_prefix("root:") {
            ("root_id=?", Some(v))
        } else {
            return Ok((0, 0));
        };
        return conn
            .query_row(
                &format!("SELECT count(*),coalesce(sum(size),0) FROM visible_files WHERE {filter}"),
                params_from_iter(value),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string());
    }
    conn.query_row(
        "SELECT file_count,bytes FROM totals WHERE scope=?",
        [scope],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map(|value| value.unwrap_or((0, 0)))
    .map_err(|e| e.to_string())
}
pub fn roots(conn: &Connection) -> Result<Vec<Root>> {
    let mut stmt = conn
        .prepare("SELECT id,path,added_at,last_scan FROM roots ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Root {
                id: r.get(0)?,
                path: r.get(1)?,
                added_at: r.get(2)?,
                last_scan: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}
pub fn file(conn: &Connection, id: i64) -> Result<FileRecord> {
    conn.query_row(
        &format!("SELECT {FILE_COLUMNS} FROM files WHERE id=?"),
        [id],
        map_file,
    )
    .map_err(|e| e.to_string())
}
pub fn stats(conn: &Connection) -> Result<LibraryStats> {
    let (count, bytes) = totals(conn, "all")?;
    let hidden = crate::visibility::has_hidden(conn)?;
    let mut stmt = conn
        .prepare(if hidden { "SELECT category,count(*),sum(size) FROM visible_files GROUP BY category" } else { "SELECT substr(scope,10),file_count,bytes FROM totals WHERE scope LIKE 'category:%' AND file_count>0" })
        .map_err(|e| e.to_string())?;
    let categories = stmt
        .query_map([], |r| {
            Ok(CategoryStat {
                category: r.get(0)?,
                count: r.get(1)?,
                bytes: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(if hidden { "SELECT source,count(*),sum(size) FROM visible_files WHERE source IS NOT NULL GROUP BY source ORDER BY count(*) DESC" } else { "SELECT substr(scope,8),file_count,bytes FROM totals WHERE scope LIKE 'source:%' AND file_count>0 ORDER BY file_count DESC" }).map_err(|e|e.to_string())?;
    let sources = stmt
        .query_map([], |r| {
            Ok(SourceStat {
                source: r.get(0)?,
                count: r.get(1)?,
                bytes: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let large = conn
        .query_row(
            "SELECT count(*) FROM visible_files WHERE size>=104857600",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let screenshots = conn
        .query_row(
            "SELECT count(*) FROM visible_files WHERE screenshot=1 AND modified<?",
            [now() - 30 * 86400],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let downloads = conn.query_row("SELECT count(*) FROM visible_files WHERE (source IS NOT NULL OR lower(replace(path,char(92),'/')) LIKE '%/downloads/%') AND modified<?", [now()-90*86400], |r|r.get(0)).map_err(|e|e.to_string())?;
    let (duplicate_groups, duplicate_bytes) = conn.query_row("SELECT count(*),coalesce(sum(size*(n-1)),0) FROM (SELECT size,count(*) n FROM visible_files WHERE hash IS NOT NULL GROUP BY hash,size HAVING count(*)>1)", [], |r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
    Ok(LibraryStats {
        count,
        bytes,
        categories,
        sources,
        large,
        screenshots,
        downloads,
        duplicate_groups,
        duplicate_bytes,
    })
}
pub fn query(conn: &Connection, q: Query) -> Result<QueryResult> {
    let simple = !q.favourites
        && q.terms.is_empty()
        && q.min_size.is_none()
        && q.max_size.is_none()
        && q.date_from.is_none()
        && q.date_to.is_none()
        && q.screenshot != Some(true)
        && q.cleanup.is_none();
    let scopes = [
        q.category.as_ref().map(|v| format!("category:{v}")),
        q.source.as_ref().map(|v| format!("source:{v}")),
        q.root_id.map(|v| format!("root:{v}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let cached_scope = if simple && scopes.len() <= 1 {
        Some(scopes.first().cloned().unwrap_or_else(|| "all".into()))
    } else {
        None
    };
    let mut clauses = vec!["1=1".to_owned()];
    let mut args: Vec<Value> = vec![];
    if q.favourites {
        clauses.push("id IN (SELECT file_id FROM favourites)".into());
    }
    for (column, value) in [("category", q.category), ("source", q.source)] {
        if let Some(v) = value {
            clauses.push(format!("{column}=?"));
            args.push(v.into());
        }
    }
    for (column, op, value) in [
        ("size", ">=", q.min_size),
        ("size", "<=", q.max_size),
        ("modified", ">=", q.date_from),
        ("modified", "<", q.date_to),
        ("root_id", "=", q.root_id),
    ] {
        if let Some(v) = value {
            clauses.push(format!("{column}{op}?"));
            args.push(v.into());
        }
    }
    if q.screenshot == Some(true) {
        clauses.push("screenshot=1".into());
    }
    match q.cleanup.as_deref() {
        Some("large") => clauses.push("size>=104857600".into()),
        Some("screenshots") => {
            clauses.push("screenshot=1 AND modified<?".into());
            args.push((now() - 30 * 86400).into());
        }
        Some("downloads") => {
            clauses.push(
                "(source IS NOT NULL OR lower(replace(path,char(92),'/')) LIKE '%/downloads/%') AND modified<?".into(),
            );
            args.push((now() - 90 * 86400).into());
        }
        _ => {}
    }
    for term in q.terms.iter().take(20) {
        if let Some(extension) = term.strip_prefix("ext:") {
            clauses.push("extension=?".into());
            args.push(extension.to_lowercase().into());
        } else if !term.is_empty() {
            // A quoted FTS prefix is safe even when filenames contain punctuation or operators.
            let fts = format!("\"{}\"*", term.replace('"', "\"\""));
            clauses.push("(id IN (SELECT rowid FROM files_fts WHERE files_fts MATCH ?) OR id IN (SELECT rowid FROM image_text_fts WHERE image_text_fts MATCH ?) OR id IN (SELECT rowid FROM document_text_fts WHERE document_text_fts MATCH ?))".into());
            args.push(fts.clone().into());
            args.push(fts.clone().into());
            args.push(fts.into());
        }
    }
    let filter = clauses.join(" AND ");
    let (total, bytes) = if let Some(scope) = cached_scope {
        totals(conn, &scope)?
    } else {
        conn.query_row(
            &format!("SELECT count(*),coalesce(sum(size),0) FROM visible_files WHERE {filter}"),
            params_from_iter(args.iter()),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?
    };
    let order = match q.sort.as_str() {
        "name" => "name COLLATE NOCASE ASC,id ASC",
        "oldest" => "modified ASC,id ASC",
        "size" => "size DESC,id DESC",
        _ => "modified DESC,id DESC",
    };
    args.push(q.limit.clamp(1, 200).into());
    args.push(q.offset.max(0).into());
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {FILE_COLUMNS} FROM visible_files WHERE {filter} ORDER BY {order} LIMIT ? OFFSET ?"
        ))
        .map_err(|e| e.to_string())?;
    let files = stmt
        .query_map(params_from_iter(args.iter()), map_file)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(QueryResult {
        files,
        total,
        bytes,
    })
}
pub fn duplicate_groups(conn: &Connection) -> Result<Vec<DuplicateGroup>> {
    let mut stmt=conn.prepare("SELECT hash,size FROM visible_files WHERE hash IS NOT NULL GROUP BY hash,size HAVING count(*)>1 ORDER BY size*(count(*)-1) DESC LIMIT 200").map_err(|e|e.to_string())?;
    let keys = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    keys.into_iter().map(|(hash,size)|{
        let mut stmt=conn.prepare(&format!("SELECT {FILE_COLUMNS} FROM visible_files WHERE hash=? AND size=? ORDER BY modified DESC,id DESC")).map_err(|e|e.to_string())?;
        let files=stmt.query_map(params![hash,size],map_file).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
        Ok(DuplicateGroup{hash,size,files})
    }).collect()
}
