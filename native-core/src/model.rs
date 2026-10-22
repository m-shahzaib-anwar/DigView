use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, Default, Debug)]
pub struct Job {
    pub revision: u64,
    pub watch_error: String,
    pub running: bool,
    pub kind: String,
    pub processed: u64,
    pub errors: u64,
    pub message: String,
    pub cancelled: bool,
    pub duration_ms: u64,
}
#[derive(Serialize, Clone, Debug)]
pub struct Root {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub last_scan: Option<i64>,
}
#[derive(Serialize, Clone, Debug)]
pub struct FileRecord {
    pub id: i64,
    pub root_id: i64,
    pub path: String,
    pub name: String,
    pub extension: String,
    pub category: String,
    pub size: i64,
    pub modified: i64,
    pub created: Option<i64>,
    pub source: Option<String>,
    pub screenshot: bool,
    pub hash: Option<String>,
    pub modified_token: String,
}
#[derive(Serialize)]
pub struct CategoryStat {
    pub category: String,
    pub count: i64,
    pub bytes: i64,
}
#[derive(Serialize)]
pub struct SourceStat {
    pub source: String,
    pub count: i64,
    pub bytes: i64,
}
#[derive(Serialize)]
pub struct LibraryStats {
    pub count: i64,
    pub bytes: i64,
    pub categories: Vec<CategoryStat>,
    pub sources: Vec<SourceStat>,
    pub large: i64,
    pub screenshots: i64,
    pub downloads: i64,
    pub duplicate_groups: i64,
    pub duplicate_bytes: i64,
}
#[derive(Serialize)]
pub struct Bootstrap {
    pub roots: Vec<Root>,
    pub stats: LibraryStats,
    pub job: Job,
    pub platform: String,
    pub ocr: String,
    pub version: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Query {
    pub favourites: bool,
    pub category: Option<String>,
    pub terms: Vec<String>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub screenshot: Option<bool>,
    pub source: Option<String>,
    pub root_id: Option<i64>,
    pub cleanup: Option<String>,
    pub sort: String,
    pub offset: i64,
    pub limit: i64,
}
#[derive(Serialize)]
pub struct QueryResult {
    pub files: Vec<FileRecord>,
    pub total: i64,
    pub bytes: i64,
}
#[derive(Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: i64,
    pub files: Vec<FileRecord>,
}
#[derive(Serialize)]
pub struct TrashFailure {
    pub id: i64,
    pub reason: String,
}
#[derive(Serialize)]
pub struct TrashResult {
    pub trashed: Vec<i64>,
    pub failed: Vec<TrashFailure>,
}
#[derive(Deserialize, Clone)]
pub struct TrashSelection {
    pub id: i64,
    pub path: String,
    pub size: i64,
    pub modified_token: String,
}
impl From<&FileRecord> for TrashSelection {
    fn from(file: &FileRecord) -> Self {
        Self {
            id: file.id,
            path: file.path.clone(),
            size: file.size,
            modified_token: file.modified_token.clone(),
        }
    }
}

pub const FILE_COLUMNS: &str = "id,root_id,path,name,extension,category,size,modified,created,source,screenshot,hash,modified_token";
pub fn map_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: row.get(0)?,
        root_id: row.get(1)?,
        path: row.get(2)?,
        name: row.get(3)?,
        extension: row.get(4)?,
        category: row.get(5)?,
        size: row.get(6)?,
        modified: row.get(7)?,
        created: row.get(8)?,
        source: row.get(9)?,
        screenshot: row.get(10)?,
        hash: row.get(11)?,
        modified_token: row.get(12)?,
    })
}
