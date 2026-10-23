//! Local text-only exact cosine search. No network or inference runtime in the core.
use crate::{db::{self, Result}, engine::{Engine, check_identity, hash_file}, model::{Query, QueryResult}};
use rusqlite::{params, Connection};
use std::{collections::HashMap, sync::atomic::{AtomicBool, Ordering}};

// Changes to model, prompts, dimensions or chunking require a new identity.
pub const ID: &str = "embeddinggemma2-bf16-68bae29d-v1-1200bytes-200overlap-64";
pub const DIM: usize = 768;
// Pinned model/prompt calibration: 14 generated demo documents, 14 positive queries,
// two no-match controls. This is a provisional relevance floor, not a probability.
pub const MIN_RELEVANCE: f32 = 0.66;
fn relevant(score: f32) -> bool { score.is_finite() && score >= MIN_RELEVANCE }

pub fn bounded_text(text: &str, max_bytes: usize) -> &str {
    let mut end=text.len().min(max_bytes);
    while !text.is_char_boundary(end) {end-=1;}
    &text[..end]
}
pub fn init(c: &Connection) -> Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS semantic_preferences(id INTEGER PRIMARY KEY CHECK(id=1),enabled INTEGER NOT NULL DEFAULT 0);
      INSERT OR IGNORE INTO semantic_preferences VALUES(1,0);
      CREATE TABLE IF NOT EXISTS semantic_documents(file_id INTEGER PRIMARY KEY REFERENCES document_text(file_id) ON DELETE CASCADE,model TEXT NOT NULL,digest TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS semantic_chunks(file_id INTEGER NOT NULL REFERENCES semantic_documents(file_id) ON DELETE CASCADE,chunk INTEGER NOT NULL,vector BLOB NOT NULL,PRIMARY KEY(file_id,chunk));
      CREATE TRIGGER IF NOT EXISTS semantic_text_changed AFTER UPDATE ON document_text BEGIN DELETE FROM semantic_documents WHERE file_id=new.file_id; END;
      CREATE TRIGGER IF NOT EXISTS semantic_name_changed AFTER UPDATE OF name ON files WHEN old.name!=new.name BEGIN DELETE FROM semantic_documents WHERE file_id=new.id; END;") .map_err(|e|e.to_string())
}
pub fn enabled(e: &Engine) -> Result<bool> {
    db::connect(&e.0.db)?.query_row("SELECT enabled FROM semantic_preferences WHERE id=1",[],|r|r.get(0)).map_err(|e|e.to_string())
}
pub fn set_enabled(e: &Engine, value: bool) -> Result<()> {
    db::connect(&e.0.db)?.execute("UPDATE semantic_preferences SET enabled=? WHERE id=1",[value]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn counts(e: &Engine) -> Result<(i64,i64)> {
    let c=db::connect(&e.0.db)?;
    c.query_row("SELECT count(s.file_id),count(*)-count(s.file_id) FROM document_text d JOIN visible_files v ON v.id=d.file_id LEFT JOIN semantic_documents s ON s.file_id=d.file_id AND s.model=? AND s.digest=d.digest WHERE d.status IN ('indexed','partial') AND length(trim(d.text))>0",[ID],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())
}
pub fn clear(e: &Engine) -> Result<()> {
    db::connect(&e.0.db)?.execute("DELETE FROM semantic_documents",[]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn normalize(mut v: Vec<f32>) -> Result<Vec<f32>> {
    if v.len()!=DIM || v.iter().any(|x|!x.is_finite()) { return Err("Invalid local embedding dimensions or values.".into()); }
    let norm=v.iter().map(|x|x*x).sum::<f32>().sqrt();
    if !norm.is_finite() || norm<1e-9 {return Err("Invalid zero embedding.".into());}
    for x in &mut v {*x/=norm;}
    Ok(v)
}
fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {Err("Semantic task cancelled. Saved vectors kept.".into())} else {Ok(())}
}
/// A document is committed only after all bounded chunks finish and its source still matches.
#[derive(Clone, Copy, Debug, Default)]
pub struct IndexProgress { pub completed: u64, pub total: u64, pub indexed: u64, pub skipped: u64 }
pub fn index(e: &Engine, cancel: &AtomicBool, mut embed: impl FnMut(String)->Result<Vec<f32>>, mut progress: impl FnMut(IndexProgress)) -> Result<()> {
    // Freeze eligible IDs once: new files wait for the next explicit run.
    let work = {
        let c=db::connect(&e.0.db)?;
        let mut stmt=c.prepare("SELECT d.file_id FROM document_text d JOIN visible_files v ON v.id=d.file_id LEFT JOIN semantic_documents s ON s.file_id=d.file_id AND s.model=? AND s.digest=d.digest WHERE s.file_id IS NULL AND d.status IN ('indexed','partial') AND length(trim(d.text))>0 ORDER BY d.file_id").map_err(|e|e.to_string())?;
        let rows=stmt.query_map([ID],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?
    };
    let mut status=IndexProgress {total:work.len() as u64,..Default::default()};
    progress(status);
    for id in work {
        cancelled(cancel)?;
        if !enabled(e)? {return Err("Semantic indexing disabled.".into());}
        let saved=index_document(e,id,cancel,&mut embed)?;
        status.completed+=1;
        if saved {status.indexed+=1;} else {status.skipped+=1;}
        progress(status);
    }
    cancelled(cancel)?;
    Ok(())
}
fn index_document(e: &Engine,id: i64,cancel: &AtomicBool,embed: &mut impl FnMut(String)->Result<Vec<f32>>) -> Result<bool> {
    let c=db::connect(&e.0.db)?;
    let next=c.query_row("SELECT v.name,d.text,d.digest FROM document_text d JOIN visible_files v ON v.id=d.file_id WHERE d.file_id=? AND d.status IN ('indexed','partial') AND length(trim(d.text))>0",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)));
    let (name,text,digest)=match next {Ok(x)=>x,Err(rusqlite::Error::QueryReturnedNoRows)=>return Ok(false),Err(err)=>return Err(err.to_string())};
        let (record,path)=match e.checked_file(id) {Ok(x)=>x,Err(_)=>return Ok(false)};
        if check_identity(&record,&path).is_err() || hash_file(&path,cancel)?!=digest {return Ok(false);}
        let mut start=0;
        let mut vectors=Vec::new();
        for _ in 0..64 {
            cancelled(cancel)?;
            let chunk=bounded_text(&text[start..],1200);
            let prompt=format!("title: {} | text: {chunk}",bounded_text(&name,256));
            let v=normalize(embed(prompt)?)?;
            vectors.push(v.iter().flat_map(|x|x.to_le_bytes()).collect::<Vec<_>>());
            if start+chunk.len()>=text.len() {break;}
            start+=chunk.len().saturating_sub(200).max(1);
            while !text.is_char_boundary(start) {start+=1;}
        }
        cancelled(cancel)?;
        let _gate=e.0.operation_gate.lock().map_err(|e|e.to_string())?;
        let (latest,latest_path)=match e.checked_file(id) {Ok(x)=>x,Err(_)=>return Ok(false)};
        if latest.path!=record.path || latest.name!=record.name || check_identity(&record,&latest_path).is_err() || hash_file(&latest_path,cancel)?!=digest {return Ok(false);}
        let mut c=db::connect(&e.0.db)?;
        let tx=c.transaction().map_err(|e|e.to_string())?;
        let same: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM document_text WHERE file_id=? AND digest=?)",params![id,digest],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !same || !enabled(e)? {return Ok(false);}
        tx.execute("DELETE FROM semantic_documents WHERE file_id=?",[id]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO semantic_documents VALUES(?,?,?)",params![id,ID,digest]).map_err(|e|e.to_string())?;
        for (i,v) in vectors.iter().enumerate() {tx.execute("INSERT INTO semantic_chunks VALUES(?,?,?)",params![id,i as i64,v]).map_err(|e|e.to_string())?;}
        cancelled(cancel)?;
        tx.commit().map_err(|e|e.to_string())?;
    Ok(true)
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct MatchPassage {pub snippet:String,pub passage:String,pub digest:String,pub start:usize,pub end:usize}
#[derive(serde::Serialize)]
pub struct MatchedResults {pub result:QueryResult,pub matches:HashMap<i64,MatchPassage>}
pub const EXPANDED_RELEVANCE:f32=0.55;
pub fn chunk_range(text:&str,chunk:usize)->Option<(usize,usize)> {
    if chunk>=64 || text.is_empty(){return None;}
    let mut start=0;
    for index in 0..=chunk {
        let end=start+bounded_text(&text[start..],1200).len();
        if index==chunk{return Some((start,end));}
        if end>=text.len(){return None;}
        start=end.saturating_sub(200).max(start+1);
        while !text.is_char_boundary(start){start+=1;}
    }
    None
}
pub fn matched_text(e:&Engine,id:i64,digest:&str)->Result<String>{
    if digest.len()!=64 || !digest.bytes().all(|c|c.is_ascii_hexdigit()){return Err("Invalid passage identity.".into());}
    let (record,path)=e.checked_file(id)?;
    check_identity(&record,&path)?;
    let c=db::connect(&e.0.db)?;
    let text:String=c.query_row("SELECT text FROM document_text WHERE file_id=? AND digest=? AND status IN ('indexed','partial')",params![id,digest],|r|r.get(0)).map_err(|_|"This passage is out of date. Refresh the search.".to_string())?;
    if hash_file(&path,&AtomicBool::new(false))?!=digest {return Err("File changed. Refresh your library and search again.".into());}
    let (latest,latest_path)=e.checked_file(id)?;
    if latest.path!=record.path{ return Err("File moved. Refresh the search.".into()); }
    check_identity(&record,&latest_path)?;
    Ok(text)
}
pub fn search(e:&Engine,q:Query,query_vector:Vec<f32>,cancel:&AtomicBool)->Result<QueryResult>{
    Ok(search_matches(e,q,query_vector,cancel,false)?.result)
}
pub fn search_matches(e: &Engine,q: Query,query_vector: Vec<f32>,cancel: &AtomicBool,broader:bool) -> Result<MatchedResults> {
    cancelled(cancel)?;
    let vector=normalize(query_vector)?;
    let c=db::connect(&e.0.db)?;
    c.execute_batch("CREATE TEMP TABLE semantic_candidates(file_id INTEGER PRIMARY KEY,score REAL NOT NULL)").map_err(|e|e.to_string())?;
    let mut scores=HashMap::<i64,f32>::new();
    let mut best_chunks=HashMap::<i64,(usize,String)>::new();
    {
        let mut stmt=c.prepare("SELECT s.file_id,s.vector,s.chunk,d.digest FROM semantic_chunks s JOIN semantic_documents m ON m.file_id=s.file_id JOIN document_text d ON d.file_id=s.file_id JOIN visible_files v ON v.id=s.file_id WHERE m.model=? AND m.digest=d.digest").map_err(|e|e.to_string())?;
        let rows=stmt.query_map([ID],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,usize>(2)?,r.get::<_,String>(3)?))).map_err(|e|e.to_string())?;
        for row in rows {
            cancelled(cancel)?;
            let (id,bytes,chunk,digest)=row.map_err(|e|e.to_string())?;
            if bytes.len()!=DIM*4 {continue;}
            let stored=bytes.chunks_exact(4).map(|b|f32::from_le_bytes([b[0],b[1],b[2],b[3]])).collect::<Vec<_>>();
            let stored=match normalize(stored) {Ok(v)=>v,Err(_)=>continue};
            let score=stored.iter().zip(&vector).map(|(d,q)|d*q).sum::<f32>();
            if score.is_finite() && scores.get(&id).is_none_or(|current|score>*current) {scores.insert(id,score);best_chunks.insert(id,(chunk,digest));}
        }
    }
    for (id,score) in scores {
        cancelled(cancel)?;
        if !(if broader {score.is_finite() && score>=EXPANDED_RELEVANCE} else {relevant(score)}) {continue;}
        // Revalidate selection, current exclusion policy, containment and filesystem identity.
        if e.checked_file(id).is_ok_and(|(r,p)|check_identity(&r,&p).is_ok()) {
            c.execute("INSERT INTO semantic_candidates VALUES(?,?)",params![id,score]).map_err(|e|e.to_string())?;
        }
    }
    // Preserve exact name/content matches, including non-text files, above meaning matches.
    let mut exact_ids=std::collections::HashSet::new();
    let mut lexical=q.clone();lexical.offset=0;lexical.limit=200;
    loop {
        cancelled(cancel)?;
        let page=db::query(&c,lexical.clone())?;
        if page.files.is_empty() {break;}
        for file in &page.files {
            if e.checked_file(file.id).is_ok_and(|(r,p)|check_identity(&r,&p).is_ok()) {
                c.execute("INSERT INTO semantic_candidates VALUES(?,2.0) ON CONFLICT(file_id) DO UPDATE SET score=2.0",[file.id]).map_err(|e|e.to_string())?;
                exact_ids.insert(file.id);
            }
        }
        lexical.offset+=page.files.len() as i64;
        if lexical.offset>=page.total {break;}
    }
    cancelled(cancel)?;
    if !enabled(e)? {return Err("Semantic search is disabled.".into());}
    let terms=q.terms.clone();
    let result=db::query_ranked(&c,q,true)?;
    let mut matches=HashMap::new();
    for file in &result.files {
        cancelled(cancel)?;
        let data=c.query_row("SELECT text,digest FROM document_text WHERE file_id=? AND status IN ('indexed','partial')",[file.id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)));
        let Ok((text,digest))=data else {continue;};
        if best_chunks.get(&file.id).is_some_and(|(_,old_digest)|old_digest!=&digest){continue;}
        let mut range=best_chunks.get(&file.id).and_then(|(chunk,_)|chunk_range(&text,*chunk));
        // Exact text matches take their own passage; filename-only hits need no invented explanation.
        if range.is_none() || exact_ids.contains(&file.id) {
        let lower=text.to_lowercase();
        let mut anchors=terms.iter().filter(|t|!t.starts_with("ext:") && t.chars().count()>=3).collect::<Vec<_>>();
        anchors.sort_by_key(|t|std::cmp::Reverse(t.len()));
        if let Some(at)=anchors.iter().filter_map(|term|lower.find(&term.to_lowercase())).find(|at|text.is_char_boundary(*at)) {
            let mut start=at.saturating_sub(100);while !text.is_char_boundary(start){start+=1;}
            range=Some((start,start+bounded_text(&text[start..],1200).len()));
        }
        }
        let Some((start,end))=range else {continue;};
        let passage=text[start..end].to_string();
        let snippet=passage.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(320).collect();
        matches.insert(file.id,MatchPassage{snippet,passage,digest,start,end});
    }
    Ok(MatchedResults{result,matches})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    use sha2::{Digest,Sha256};
    fn seed_text(e: &Engine) {
        let c=db::connect(&e.0.db).unwrap();
        let files=db::query(&c,Query{limit:200,..Default::default()}).unwrap().files;
        for f in files {
            let bytes=fs::read(&f.path).unwrap();
            c.execute("INSERT OR IGNORE INTO document_text(file_id,size,modified_token,digest,text,status,version) VALUES(?,?,?,?,?,'indexed',1)",params![f.id,f.size,f.modified_token,format!("{:x}",Sha256::digest(&bytes)),String::from_utf8(bytes).unwrap()]).unwrap();
        }
    }
    fn fixture() -> (tempfile::TempDir,Engine,std::path::PathBuf) {
        let t=tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root=t.path().join("selected");fs::create_dir(&root).unwrap();
        fs::write(root.join("first.txt"),"first document").unwrap();
        fs::write(root.join("second.txt"),"second document").unwrap();
        let e=Engine::new(&t.path().join("data")).unwrap();e.add_root(&root).unwrap();e.scan_mode(false).unwrap();seed_text(&e);set_enabled(&e,true).unwrap();
        (t,e,root)
    }
    fn vector() -> Vec<f32> {let mut v=vec![0.;DIM];v[0]=1.;v}
    #[test]
    fn progress_freezes_work_and_accounts_for_removed_files_without_claiming_they_were_indexed() {
        let (_t,e,root)=fixture();let cancel=AtomicBool::new(false);let mut events=Vec::new();let mut changed=false;
        index(&e,&cancel,|prompt| {
            if !changed {
                changed=true;
                // The first prompt identifies which file is currently being embedded.
                let other=if prompt.contains("first.txt") {"second.txt"} else {"first.txt"};
                fs::remove_file(root.join(other)).unwrap();
                fs::write(root.join("new.txt"),"arrived during indexing").unwrap();
                e.scan_mode(false).unwrap();seed_text(&e);
            }
            Ok(vector())
        },|p|events.push(p)).unwrap();
        assert_eq!(events.iter().map(|p|(p.completed,p.total)).collect::<Vec<_>>(),vec![(0,2),(1,2),(2,2)]);
        let last=events.last().unwrap();assert_eq!((last.indexed,last.skipped),(1,1));
        assert_eq!(counts(&e).unwrap(),(1,1)); // The new document waits for the next run.
    }
    #[test]
    fn cancellation_keeps_completed_progress_and_never_marks_unfinished_files_done() {
        let (_t,e,_)=fixture();let cancel=AtomicBool::new(false);let mut events=Vec::new();
        let result=index(&e,&cancel,|_|Ok(vector()),|p| {events.push(p);if p.completed==1 {cancel.store(true,Ordering::Relaxed);}});
        assert!(result.is_err());let last=events.last().unwrap();assert_eq!((last.completed,last.total,last.indexed),(1,2,1));
        assert_eq!(counts(&e).unwrap(),(1,1));
    }
    #[test]
    fn already_indexed_files_do_not_inflate_the_next_runs_total() {
        let (_t,e,_)=fixture();let cancel=AtomicBool::new(false);
        index(&e,&cancel,|_|Ok(vector()),|_|{}).unwrap();
        let mut events=Vec::new();index(&e,&cancel,|_|panic!("no pending documents"),|p|events.push(p)).unwrap();
        assert_eq!(events.len(),1);assert_eq!((events[0].completed,events[0].total),(0,0));
    }
    #[test]
    fn exact_matches_keep_priority_and_pagination_with_automatic_meaning_results() {
        let (_t,e,_)=fixture();let cancel=AtomicBool::new(false);
        index(&e,&cancel,|_|Ok(vector()),|_|{}).unwrap();
        let first=search(&e,Query{terms:vec!["second".into()],limit:1,..Default::default()},vector(),&cancel).unwrap();
        assert_eq!(first.total,2);assert_eq!(first.files[0].name,"second.txt");
        let next=search(&e,Query{terms:vec!["second".into()],offset:1,limit:1,..Default::default()},vector(),&cancel).unwrap();
        assert_eq!(next.files[0].name,"first.txt");
    }

    #[test]
    fn relevance_floor_is_inclusive_and_rejects_invalid_scores() {
        assert!(relevant(MIN_RELEVANCE));
        assert!(!relevant(MIN_RELEVANCE-0.00001));
        assert!(!relevant(f32::NAN));assert!(!relevant(f32::INFINITY));
        assert!(!relevant(-0.5));
    }
    #[test]
    fn measured_demo_primary_matches_survive_and_no_match_controls_stay_empty() {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("fixtures/embeddinggemma2-relevance.json")).unwrap();
        let cases=fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(),16);
        for case in cases {
            let kept=case["scores"].as_array().unwrap().iter().filter(|r|relevant(r[1].as_f64().unwrap() as f32)).map(|r|r[0].as_str().unwrap()).collect::<Vec<_>>();
            if let Some(primary)=case["targets"].as_array().unwrap().first() {
                assert!(kept.contains(&primary.as_str().unwrap()),"lost expected target: {}",case["query"]);
            } else {assert!(kept.is_empty(),"no-match query leaked results: {}",case["query"]);}
        }
        let kept=cases[0]["scores"].as_array().unwrap().iter().filter(|r|relevant(r[1].as_f64().unwrap() as f32)).map(|r|r[0].as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(kept,vec!["record-014.pdf","entry-023.md"]);
    }
    #[test]
    fn relevance_filters_counts_before_pagination_but_keeps_low_score_exact_matches() {
        let (_t,e,_)=fixture();let cancel=AtomicBool::new(false);
        index(&e,&cancel,|_|Ok(vector()),|_|{}).unwrap();
        let c=db::connect(&e.0.db).unwrap();
        for (name,score) in [("first.txt",0.70f32),("second.txt",0.50f32)] {
            let mut v=vec![0f32;DIM];v[0]=score;v[1]=(1.-score*score).sqrt();
            let bytes=v.iter().flat_map(|x|x.to_le_bytes()).collect::<Vec<_>>();
            c.execute("UPDATE semantic_chunks SET vector=? WHERE file_id=(SELECT id FROM files WHERE name=?)",params![bytes,name]).unwrap();
        }
        let q=||Query{terms:vec!["membership".into()],limit:1,..Default::default()};
        let result=search(&e,q(),vector(),&cancel).unwrap();
        assert_eq!(result.total,1);assert_eq!(result.files[0].name,"first.txt");assert_eq!(result.bytes,result.files[0].size);
        let mut later=q();later.offset=1;
        let result=search(&e,later,vector(),&cancel).unwrap();assert!(result.files.is_empty());assert_eq!(result.total,1);
        let exact=search(&e,Query{terms:vec!["second".into()],limit:200,..Default::default()},vector(),&cancel).unwrap();
        assert_eq!(exact.total,2);assert_eq!(exact.files[0].name,"second.txt");
        let mut unrelated=vec![0f32;DIM];unrelated[2]=1.;
        let empty=search(&e,q(),unrelated.clone(),&cancel).unwrap();assert_eq!((empty.total,empty.bytes),(0,0));assert!(empty.files.is_empty());
        let exact=search(&e,Query{terms:vec!["second".into()],limit:200,..Default::default()},unrelated,&cancel).unwrap();assert_eq!(exact.total,1);assert_eq!(exact.files[0].name,"second.txt");
        let mut filtered=q();filtered.category=Some("images".into());assert_eq!(search(&e,filtered,vector(),&cancel).unwrap().total,0);
    }

    #[test]
    fn broader_results_are_explicit_and_preserve_filters_and_pagination() {
        let (_t,e,_)=fixture();let cancel=AtomicBool::new(false);index(&e,&cancel,|_|Ok(vector()),|_|{}).unwrap();
        let c=db::connect(&e.0.db).unwrap();
        for (name,score) in [("first.txt",0.7f32),("second.txt",0.6f32)] {
            let mut v=vec![0f32;DIM];v[0]=score;v[1]=(1.-score*score).sqrt();let bytes=v.iter().flat_map(|x|x.to_le_bytes()).collect::<Vec<_>>();
            c.execute("UPDATE semantic_chunks SET vector=? WHERE file_id=(SELECT id FROM files WHERE name=?)",params![bytes,name]).unwrap();
        }
        let q=||Query{terms:vec!["payback".into()],limit:1,..Default::default()};
        assert_eq!(search_matches(&e,q(),vector(),&cancel,false).unwrap().result.total,1);
        let broad=search_matches(&e,q(),vector(),&cancel,true).unwrap();assert_eq!(broad.result.total,2);assert_eq!(broad.result.files.len(),1);assert_eq!(broad.matches.len(),1);
        let mut next=q();next.offset=1;assert_eq!(search_matches(&e,next,vector(),&cancel,true).unwrap().result.files[0].name,"second.txt");
        let mut filtered=q();filtered.category=Some("images".into());assert_eq!(search_matches(&e,filtered,vector(),&cancel,true).unwrap().result.total,0);
    }
    #[test]
    fn matched_passage_tracks_the_best_chunk_and_rejects_changed_or_removed_sources() {
        let (_t,e,root)=fixture();let cancel=AtomicBool::new(false);
        let content=format!("{}\nThe reimbursement paragraph says a cancelled service is repaid to the buyer.\n{}","Unrelated background. ".repeat(70),"More unrelated detail. ".repeat(70));
        fs::write(root.join("first.txt"),&content).unwrap();e.scan_mode(false).unwrap();seed_text(&e);
        index(&e,&cancel,|prompt|{let mut v=vector();if !prompt.contains("reimbursement") {v[0]=0.;v[1]=1.;}Ok(v)},|_|{}).unwrap();
        let found=search_matches(&e,Query{terms:vec!["payback".into()],limit:200,..Default::default()},vector(),&cancel,false).unwrap();
        assert_eq!(found.result.total,1);let file=&found.result.files[0];let m=&found.matches[&file.id];
        assert!(m.passage.contains("reimbursement"));assert!(m.snippet.chars().count()<=320);assert_eq!(&content[m.start..m.end],m.passage);
        assert_eq!(matched_text(&e,file.id,&m.digest).unwrap(),content);
        fs::write(root.join("first.txt"),"different source").unwrap();assert!(matched_text(&e,file.id,&m.digest).is_err());
        e.remove_root(file.root_id).unwrap();assert!(matched_text(&e,file.id,&m.digest).is_err());
    }
    #[test]
    fn chunk_navigation_handles_unicode_and_invalid_indices() {
        let text="😀é漢字".repeat(600);
        for index in 0..8 {if let Some((a,b))=chunk_range(&text,index){assert!(text.is_char_boundary(a));assert!(text.is_char_boundary(b));assert!(b-a<=1200);}}
        assert!(chunk_range("",0).is_none());assert!(chunk_range("short",1).is_none());assert!(chunk_range("text",64).is_none());
    }

}

/// Fingerprint pending inputs so arrivals during a run are queued and unchanged skips wait.
pub fn pending_key(e: &Engine) -> Result<String> {
    use sha2::{Digest,Sha256};
    let c=db::connect(&e.0.db)?;
    let mut stmt=c.prepare("SELECT d.file_id,d.digest,v.name,v.modified_token,coalesce(r.last_scan,0) FROM document_text d JOIN visible_files v ON v.id=d.file_id JOIN roots r ON r.id=v.root_id LEFT JOIN semantic_documents s ON s.file_id=d.file_id AND s.model=? AND s.digest=d.digest WHERE s.file_id IS NULL AND d.status IN ('indexed','partial') AND length(trim(d.text))>0 ORDER BY d.file_id").map_err(|e|e.to_string())?;
    let mut rows=stmt.query([ID]).map_err(|e|e.to_string())?;let mut hash=Sha256::new();
    hash.update(e.0.policy_revision.load(Ordering::Relaxed).to_le_bytes());
    while let Some(row)=rows.next().map_err(|e|e.to_string())? {
        let item=(row.get::<_,i64>(0).map_err(|e|e.to_string())?,row.get::<_,String>(1).map_err(|e|e.to_string())?,row.get::<_,String>(2).map_err(|e|e.to_string())?,row.get::<_,String>(3).map_err(|e|e.to_string())?,row.get::<_,i64>(4).map_err(|e|e.to_string())?);
        hash.update(serde_json::to_vec(&item).map_err(|e|e.to_string())?);
    }
    Ok(format!("{:x}",hash.finalize()))
}
/// Pure orchestration decision; calling this never starts a scan or reads file contents.
#[derive(Debug,PartialEq)]
pub enum AutoStep { Wait, EnableText, ReadText, Embed, Ready }
pub fn auto_step(authorized:bool, busy:bool, text_enabled:bool, text_pending:i64, semantic_pending:i64, changed:bool) -> AutoStep {
    if !authorized || busy {AutoStep::Wait}
    else if !text_enabled {AutoStep::EnableText}
    else if text_pending>0 {AutoStep::ReadText}
    else if semantic_pending>0 && changed {AutoStep::Embed}
    else if semantic_pending==0 {AutoStep::Ready}
    else {AutoStep::Wait}
}
#[cfg(test)]
mod automatic_tests {
    use super::*;
    #[test] fn consent_pause_and_active_library_work_block_automatic_processing() {
        assert_eq!(auto_step(false,false,false,5,5,true),AutoStep::Wait);
        assert_eq!(auto_step(true,true,true,0,5,true),AutoStep::Wait);
    }
    #[test] fn download_to_ready_and_new_text_follow_the_same_pipeline() {
        assert_eq!(auto_step(true,false,false,3,0,true),AutoStep::EnableText);
        assert_eq!(auto_step(true,false,true,3,0,true),AutoStep::ReadText);
        assert_eq!(auto_step(true,false,true,0,3,true),AutoStep::Embed);
        assert_eq!(auto_step(true,false,true,0,0,true),AutoStep::Ready);
        assert_eq!(auto_step(true,false,true,2,0,true),AutoStep::ReadText);
    }
    #[test] fn unchanged_skipped_work_waits_until_reconciliation_or_retry() {
        assert_eq!(auto_step(true,false,true,0,2,false),AutoStep::Wait);
        assert_eq!(auto_step(true,false,true,0,2,true),AutoStep::Embed);
    }
}
