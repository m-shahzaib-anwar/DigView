//! Optional managed local EmbeddingGemma 2. Network is used only for fixed public assets.
use digview_core::{db::{self, Result}, engine::Engine, model::{Query, QueryResult}, semantic as core};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::{self, File}, io::{Read, Write}, net::TcpListener, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{Arc, Weak, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}, time::{Duration, Instant}};
use tauri::{Manager, State};
const MODEL_URL: &str = "https://huggingface.co/ggml-org/embeddinggemma-2-GGUF/resolve/58faf90940886df2d62be555c8765d00dac362c5/embeddinggemma-2-BF16.gguf";
const MODEL_HASH: &str = "68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216";
const MODEL_BYTES: u64 = 557950176;
const RUNTIME_URL: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b11835/llama-b11835-bin-win-cpu-x64.zip";
const RUNTIME_HASH: &str = "256e606f93b7f7dfb440e8d78bdbac34184c3b45ae4defead4e072a84afe63e4";
const RUNTIME_BYTES: u64 = 24081155;
const REQUIRED_BYTES: u64 = 1024*1024*1024;
#[derive(Clone, Default)]
pub struct Semantic(Arc<Inner>);
#[derive(Default)]
struct Inner { control: Mutex<()>, interrupting: AtomicUsize, busy: AtomicBool, cancel: AtomicBool, stopping: AtomicBool, blocked: AtomicBool, attempted: Mutex<String>, progress: Mutex<Progress>, child: Mutex<Option<Child>>, runtime: Mutex<WarmWorker<Server>> }
#[derive(Default, Clone, Serialize)]
struct Progress { phase: String, downloaded: u64, processed: u64, total: u64, indexed: u64, skipped: u64, task: String, finished: bool, message: String }
#[derive(Serialize)]
pub struct Status {
    supported: bool, enabled: bool, installed: bool, busy: bool, indexed: i64, pending: i64,
    download_bytes: u64, required_bytes: u64, available_bytes: Option<u64>, progress: Progress,
}
fn directory(e: &Engine) -> PathBuf { e.0.app_data.join("semantic-embeddinggemma2-v1") }
fn owned_directory(e: &Engine) -> Result<PathBuf> {
    let path=directory(e);
    fs::create_dir_all(&path).map_err(|e|e.to_string())?;
    let canonical=fs::canonicalize(&path).map_err(|e|e.to_string())?;
    if canonical.parent()!=Some(e.0.app_data.as_path()) {return Err("Model cache must remain in DigView application data.".into());}
    Ok(canonical)
}
fn supported() -> bool { cfg!(all(windows,target_arch="x86_64")) }
fn installed(e: &Engine) -> bool {
    let d=directory(e);
    fs::metadata(d.join("model.gguf")).is_ok_and(|m|m.len()==MODEL_BYTES) && fs::metadata(d.join("runtime.zip")).is_ok_and(|m|m.len()==RUNTIME_BYTES)
}
#[cfg(windows)]
fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name="kernel32")]
    extern "system" { fn GetDiskFreeSpaceExW(path: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32; }
    let wide=path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
    let mut available=0;
    if unsafe {GetDiskFreeSpaceExW(wide.as_ptr(),&mut available,std::ptr::null_mut(),std::ptr::null_mut())}!=0 {Some(available)} else {None}
}
#[cfg(not(windows))]
fn free_bytes(_: &Path) -> Option<u64> {None}
fn progress(s: &Semantic, phase: &str, message: &str) {
    let mut p=s.0.progress.lock().unwrap_or_else(|e|e.into_inner()); p.phase=phase.into(); p.message=message.into();
}
fn check_cancel(s: &Semantic) -> Result<()> {
    if s.0.cancel.load(Ordering::Acquire) { Err("Cancelled. Retry when ready; existing search remains available.".into()) } else {Ok(())}
}
fn verify(path: &Path, size: u64, expected: &str, s: &Semantic) -> Result<()> {
    let mut f=File::open(path).map_err(|_|"Model files are missing. Download or retry in Settings.")?;
    if f.metadata().map_err(|e|e.to_string())?.len()!=size {return Err("Model download is incomplete. Retry in Settings.".into());}
    let mut hash=Sha256::new(); let mut buf=[0u8;131072];
    loop {check_cancel(s)?; let n=f.read(&mut buf).map_err(|e|e.to_string())?; if n==0 {break;} hash.update(&buf[..n]);}
    if format!("{:x}",hash.finalize())!=expected {return Err("The downloaded model or engine could not be verified. Choose Remove model & text index, then Download model to try again.".into());}
    Ok(())
}
fn download(s: &Semantic, e: &Engine) -> Result<()> {
    let dir=owned_directory(e)?;
    if free_bytes(&e.0.app_data).is_some_and(|n|n<REQUIRED_BYTES) {return Err("At least 1 GiB of free disk space is required for installation.".into());}
    let client=reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).read_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let u=attempt.url(); let h=u.host_str().unwrap_or("");
            let allowed=["huggingface.co","hf.co","github.com","githubusercontent.com"].iter().any(|d|h==*d || h.ends_with(&format!(".{d}")));
            if attempt.previous().len()<10 && u.scheme()=="https" && allowed {attempt.follow()} else {attempt.stop()}
        })).build().map_err(|_|"Could not initialize model download.")?;
    let mut completed=0;
    for (url,name,size,hash) in [(MODEL_URL,"model.gguf",MODEL_BYTES,MODEL_HASH),(RUNTIME_URL,"runtime.zip",RUNTIME_BYTES,RUNTIME_HASH)] {
        check_cancel(s)?;
        let target=dir.join(name);
        if verify(&target,size,hash,s).is_ok() {completed+=size; s.0.progress.lock().unwrap_or_else(|e|e.into_inner()).downloaded=completed; continue;}
        check_cancel(s)?;
        progress(s,"downloading",if name=="model.gguf" {"Downloading EmbeddingGemma 2 for local text search…"} else {"Downloading the engine that runs the model on this computer…"});
        let partial=dir.join(format!("{name}.part"));
        let result=tauri::async_runtime::block_on(async {
            let mut response=client.get(url).send().await.map_err(|_|"Download interrupted or offline. Retry when connected.")?;
            if !response.status().is_success() {return Err("Model download server refused the request. Retry later.".into());}
            if response.content_length().is_some_and(|n|n!=size) {return Err("Unexpected model download size.".into());}
            let mut file=File::create(&partial).map_err(|e|e.to_string())?;
            let mut bytes=0u64; let mut sha=Sha256::new();
            while let Some(chunk)=response.chunk().await.map_err(|_|"Download interrupted. Retry when connected.")? {
                check_cancel(s)?; bytes+=chunk.len() as u64;
                if bytes>size {return Err("Model download exceeded the expected size.".into());}
                file.write_all(&chunk).map_err(|_|"Could not save model download. Check free disk space.")?; sha.update(&chunk);
                s.0.progress.lock().unwrap_or_else(|e|e.into_inner()).downloaded=completed+bytes;
            }
            check_cancel(s)?;
            if bytes!=size || format!("{:x}",sha.finalize())!=hash {return Err("Model download failed its integrity check. Retry.".into());}
            file.sync_all().map_err(|e|e.to_string())?; drop(file);
            // These are application-owned cache artifacts, never library originals.
            if target.exists() {fs::remove_file(&target).map_err(|e|e.to_string())?;}
            fs::rename(&partial,&target).map_err(|e|e.to_string())?;
            Ok::<(),String>(())
        });
        if result.is_err() {let _=fs::remove_file(&partial);} result?; completed+=size;
    }
    check_cancel(s)?;
    // Verify/extract now, but do not start inference or index user files during installation.
    extract_runtime(&dir,s)?;
    fs::write(dir.join("NOTICES.txt"),include_str!("../EMBEDDINGGEMMA_NOTICES.txt")).map_err(|e|e.to_string())?;
    core::set_enabled(e,true)?;
    if let Err(err)=check_cancel(s) {core::set_enabled(e,false)?;return Err(err);}
    s.0.progress.lock().unwrap_or_else(|e|e.into_inner()).downloaded=MODEL_BYTES+RUNTIME_BYTES;
    progress(s,"downloaded","Download complete. Preparing your selected folders automatically…");
    Ok(())
}
fn extract_runtime(dir: &Path,s: &Semantic) -> Result<PathBuf> {
    verify(&dir.join("runtime.zip"),RUNTIME_BYTES,RUNTIME_HASH,s)?;
    let mut zip=zip::ZipArchive::new(File::open(dir.join("runtime.zip")).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let output=dir.join("runtime"); fs::create_dir_all(&output).map_err(|e|e.to_string())?;
    if fs::canonicalize(&output).map_err(|e|e.to_string())?.parent()!=Some(dir) || fs::symlink_metadata(&output).map_err(|e|e.to_string())?.file_type().is_symlink() {return Err("Unsafe runtime directory.".into());}
    let mut seen=std::collections::HashSet::new(); let mut total=0u64;
    for i in 0..zip.len() {
        check_cancel(s)?;
        let mut entry=zip.by_index(i).map_err(|e|e.to_string())?;
        let safe=entry.enclosed_name().ok_or("Unsafe runtime archive path.")?;
        let name=safe.file_name().and_then(|s|s.to_str()).ok_or("Invalid runtime archive filename.")?.to_owned();
        if entry.is_dir() {continue;}
        if name!="llama-server.exe" && !name.ends_with(".dll") && !name.ends_with(".txt") {continue;}
        total+=entry.size(); if total>512*1024*1024 || !seen.insert(name.clone()) {return Err("Unexpected runtime archive contents.".into());}
        let target=output.join(name);
        if target.exists() && fs::symlink_metadata(&target).map_err(|e|e.to_string())?.file_type().is_symlink() {return Err("Unsafe runtime file.".into());}
        let mut file=File::create(&target).map_err(|e|e.to_string())?;
        std::io::copy(&mut entry,&mut file).map_err(|e|e.to_string())?;
    }
    let server=output.join("llama-server.exe");
    if !server.is_file() {return Err("Local runtime archive is missing its server.".into());}
    Ok(server)
}
// Keep the verified worker warm briefly; idle expiry releases both model RAM and query text.
const WORKER_IDLE: Duration = Duration::from_secs(120);
struct WarmWorker<T> { worker: Option<T>, used: Option<Instant> }
impl<T> Default for WarmWorker<T> { fn default() -> Self {Self {worker:None,used:None}} }
impl<T> WarmWorker<T> {
    fn get_or_start(&mut self, now: Instant, healthy: impl FnOnce(&mut T)->bool, start: impl FnOnce()->Result<T>) -> Result<&mut T> {
        if self.worker.as_mut().is_some_and(|worker|!healthy(worker)) {self.worker.take();}
        if self.worker.is_none() {self.worker=Some(start()?);}
        self.used=Some(now);
        Ok(self.worker.as_mut().unwrap())
    }
    fn expire(&mut self, now: Instant) {
        if self.used.is_some_and(|used|now.saturating_duration_since(used)>=WORKER_IDLE) {self.worker.take();self.used=None;}
    }
}
fn with_server<T>(e:&Engine,s:&Semantic,run:impl FnOnce(&mut Server)->Result<T>)->Result<T> {
    let mut cache=s.0.runtime.lock().unwrap_or_else(|e|e.into_inner());
    let server=cache.get_or_start(Instant::now(), |_| {
        s.0.child.lock().unwrap_or_else(|e|e.into_inner()).as_mut().is_some_and(|child|matches!(child.try_wait(),Ok(None)))
    }, ||Server::start(e,s))?;
    let result=run(server);
    cache.used=Some(Instant::now());
    if result.is_err() || s.0.cancel.load(Ordering::Acquire) {cache.worker.take();}
    result
}
struct Server {state: Weak<Inner>, url: String, key: String, client: reqwest::Client,
    last_query: Option<(String,Vec<f32>)>,
    #[cfg(windows)]
    _job: digview_core::document_index::MemoryJob,
}
fn stop(s: &Semantic) {
    if let Some(mut child)=s.0.child.lock().unwrap_or_else(|e|e.into_inner()).take() {let _=child.kill(); let _=child.wait();}
}
impl Drop for Server {fn drop(&mut self) {if let Some(state)=self.state.upgrade() {stop(&Semantic(state));}}}
impl Server {
    fn start(e: &Engine,s: &Semantic) -> Result<Self> {
        progress(s,"loading","Checking model files and starting the local search engine…");
        let dir=owned_directory(e)?; verify(&dir.join("model.gguf"),MODEL_BYTES,MODEL_HASH,s)?;
        let executable=extract_runtime(&dir,s)?;
        let listener=TcpListener::bind("127.0.0.1:0").map_err(|e|e.to_string())?;
        let port=listener.local_addr().map_err(|e|e.to_string())?.port();
        let mut random=[0u8;32]; getrandom::fill(&mut random).map_err(|_|"Could not start the private local search engine.")?;
        let key=random.iter().map(|b|format!("{b:02x}")).collect::<String>();
        let mut cmd=Command::new(executable);
        cmd.current_dir(dir.join("runtime")).args(["--model"]).arg(dir.join("model.gguf"))
            .args(["--host","127.0.0.1","--port",&port.to_string(),"--api-key",&key,"--embedding","--pooling","mean","--ctx-size","2048","--batch-size","2048","--ubatch-size","2048","--parallel","1","--threads","2","--n-gpu-layers","0","--no-webui"])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)] {use std::os::windows::process::CommandExt; cmd.creation_flags(0x08000000);}
        drop(listener);
        let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(120)).build().map_err(|e|e.to_string())?;
        let mut child=cmd.spawn().map_err(|_|"Could not start local CPU inference. Remove/download the model again if runtime files are missing.")?;
        #[cfg(windows)]
        let job=match digview_core::document_index::worker_job(&child, 3 * 1024 * 1024 * 1024) {
            Ok(job)=>job, Err(_)=>{let _=child.kill();let _=child.wait();return Err("Could not contain the local model worker.".into());}
        };
        *s.0.child.lock().unwrap_or_else(|e|e.into_inner())=Some(child);
        let server=Self {state:Arc::downgrade(&s.0),last_query:None,url:format!("http://127.0.0.1:{port}"),key,client,
            #[cfg(windows)]
            _job:job,
        };
        let started=Instant::now();
        while started.elapsed()<Duration::from_secs(120) {
            check_cancel(s)?;
            if s.0.child.lock().unwrap_or_else(|e|e.into_inner()).as_mut().map_or(true, |c| c.try_wait().ok().flatten().is_some()) {return Err("Local text model failed to start. Existing search remains available.".into());}
            let ready=tauri::async_runtime::block_on(server.client.get(format!("{}/health",server.url)).bearer_auth(&server.key).timeout(Duration::from_secs(1)).send());
            if ready.is_ok_and(|r|r.status().is_success()) {return Ok(server);}
            std::thread::sleep(Duration::from_millis(200));
        }
        Err("Local text model startup timed out. Existing search remains available.".into())
    }
    fn query_vector(&mut self,text:String,s:&Semantic)->Result<Vec<f32>> {
        check_cancel(s)?;
        if let Some((previous,vector))=&self.last_query {if previous==&text {return Ok(vector.clone());}}
        let vector=self.embed(text.clone(),s)?;
        self.last_query=Some((text,vector.clone()));
        Ok(vector)
    }
    fn embed(&self,text: String,s: &Semantic) -> Result<Vec<f32>> {
        check_cancel(s)?;
        let body=serde_json::to_vec(&serde_json::json!({"input":text,"encoding_format":"float"})).map_err(|e|e.to_string())?;
        tauri::async_runtime::block_on(async {
            let mut response=self.client.post(format!("{}/v1/embeddings",self.url)).bearer_auth(&self.key).header("Content-Type","application/json").body(body).send().await.map_err(|_|"Local embedding request failed or timed out.")?;
            if !response.status().is_success() {return Err("Local model could not embed this text.".into());}
            let mut bytes=Vec::new();
            while let Some(chunk)=response.chunk().await.map_err(|_|"Local embedding response failed.")? {check_cancel(s)?; if bytes.len()+chunk.len()>128*1024 {return Err("Unexpected local embedding response size.".into());} bytes.extend_from_slice(&chunk);}
            let value: serde_json::Value=serde_json::from_slice(&bytes).map_err(|_|"Invalid local embedding response.")?;
            let a=value["data"][0]["embedding"].as_array().ok_or("Missing local text embedding.")?;
            let v=a.iter().map(|x|x.as_f64().map(|v|v as f32).ok_or_else(||"Invalid embedding number.".to_string())).collect::<Result<Vec<_>>>()?;
            core::normalize(v)
        })
    }
}
struct Busy(Semantic);
impl Drop for Busy {fn drop(&mut self) {self.0.0.busy.store(false,Ordering::Release);}}
// Fence asynchronous pause/cancel cleanup against new queries resetting cancellation.
struct Interrupt(Semantic);
impl Drop for Interrupt {fn drop(&mut self) {self.0.0.interrupting.fetch_sub(1,Ordering::AcqRel);}}
fn interrupt(s:&Semantic)->Interrupt {
    let _gate=s.0.control.lock().unwrap_or_else(|e|e.into_inner());
    s.0.interrupting.fetch_add(1,Ordering::AcqRel);
    s.0.cancel.store(true,Ordering::Release);
    Interrupt(s.clone())
}
fn acquire(s: &Semantic) -> Result<Busy> {
    let _gate=s.0.control.lock().unwrap_or_else(|e|e.into_inner());
    if s.0.stopping.load(Ordering::Acquire) || s.0.interrupting.load(Ordering::Acquire)>0 {
        return Err("AI search is stopping. Wait for the current action to finish.".into());
    }
    s.0.busy.compare_exchange(false,true,Ordering::AcqRel,Ordering::Relaxed).map_err(|_|"The model is already busy. Wait for the current task to finish or choose Cancel current task.")?;
    s.0.cancel.store(false,Ordering::Release); Ok(Busy(s.clone()))
}
fn search_failure(s:&Semantic,error:&str) {
    let _gate=s.0.control.lock().unwrap_or_else(|e|e.into_inner());
    // A pending control action owns the final paused/cancelled/shutdown state.
    if s.0.interrupting.load(Ordering::Acquire)>0 || s.0.stopping.load(Ordering::Acquire) {return;}
    if s.0.cancel.load(Ordering::Acquire) {progress(s,"cancelled","AI search interrupted. Existing search remains available.");}
    else {s.0.blocked.store(true,Ordering::Release);progress(s,"error",error);}
}
#[tauri::command]
pub async fn semantic_status(state: State<'_,Engine>,sem: State<'_,Semantic>) -> Result<Status> {
    let e=state.inner().clone(); let s=sem.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (indexed,pending)=core::counts(&e)?;
        let snapshot=s.0.progress.lock().unwrap_or_else(|e|e.into_inner()).clone();
        Ok::<Status,String>(Status {supported:supported(),enabled:core::enabled(&e)?,installed:installed(&e),busy:s.0.busy.load(Ordering::Acquire),indexed,pending,download_bytes:MODEL_BYTES+RUNTIME_BYTES,required_bytes:REQUIRED_BYTES,available_bytes:free_bytes(&e.0.app_data),progress:snapshot})
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn semantic_action(state: State<'_,Engine>,sem: State<'_,Semantic>,action: String) -> Result<()> {
    let e=state.inner().clone(); let s=sem.inner().clone();
    if action=="cancel" {let interrupt=interrupt(&s); return tauri::async_runtime::spawn_blocking(move || {let _interrupt=interrupt;stop(&s);s.0.runtime.lock().unwrap_or_else(|e|e.into_inner()).worker.take();Ok::<(),String>(())}).await.map_err(|e|e.to_string())?;}
    if action=="disable" || action=="pause" {let interrupt=interrupt(&s); return tauri::async_runtime::spawn_blocking(move || {let _interrupt=interrupt;stop(&s);s.0.runtime.lock().unwrap_or_else(|e|e.into_inner()).worker.take();core::set_enabled(&e,false)?;e.set_document_index_enabled(false)?;progress(&s,"paused","AI search paused");e.0.revision.fetch_add(1,Ordering::Relaxed);Ok::<(),String>(())}).await.map_err(|e|e.to_string())?;}
    launch(e,s,action)
}
fn launch(e: Engine,s: Semantic,action: String) -> Result<()> {
    if !supported() {return Err("Local semantic search currently supports Windows x64 only.".into());}
    if !["download","enable","index","remove"].contains(&action.as_str()) {return Err("Unknown semantic action.".into());}
    let guard=acquire(&s)?;
    s.0.blocked.store(false,Ordering::Release);
    s.0.attempted.lock().unwrap_or_else(|e|e.into_inner()).clear();
    if action=="index" && e.job().running {return Err("Wait for document content indexing or the current library task to finish, then choose Prepare text index again.".into());}
    *s.0.progress.lock().unwrap_or_else(|e|e.into_inner())=Progress {task:action.clone(),..Default::default()};
    progress(&s,"preparing","Preparing your request…");
    tauri::async_runtime::spawn_blocking(move || {
        let _guard=guard;
        let result=(|| -> Result<()> {
            match action.as_str() {
                "download"=>{s.0.runtime.lock().unwrap_or_else(|e|e.into_inner()).worker.take();download(&s,&e)},
                "enable"=> {if !installed(&e) {return Err("Download the model first.".into());} core::set_enabled(&e,true)?; if let Err(err)=check_cancel(&s) {core::set_enabled(&e,false)?;return Err(err);} progress(&s,"ready","AI search resumed. Checking your selected folders…"); Ok(())},
                "remove"=> {
                    s.0.runtime.lock().unwrap_or_else(|e|e.into_inner()).worker.take();
                    core::set_enabled(&e,false)?; core::clear(&e)?;
                    let dir=owned_directory(&e)?;
                    // Only fixed application-owned artifacts are removed; no recursive/user paths.
                    for name in ["model.gguf","runtime.zip","model.gguf.part","runtime.zip.part","NOTICES.txt"] {let path=dir.join(name); if path.exists() {fs::remove_file(path).map_err(|e|e.to_string())?;}}
                    let runtime=dir.join("runtime");
                    if runtime.is_dir() && fs::canonicalize(&runtime).map_err(|e|e.to_string())?.parent()==Some(dir.as_path()) && !fs::symlink_metadata(&runtime).map_err(|e|e.to_string())?.file_type().is_symlink() {
                        for item in fs::read_dir(&runtime).map_err(|e|e.to_string())? {let item=item.map_err(|e|e.to_string())?; if item.file_type().map_err(|e|e.to_string())?.is_file() {fs::remove_file(item.path()).map_err(|e|e.to_string())?;}}
                        let _=fs::remove_dir(runtime);
                    }
                    progress(&s,"removed","Model and prepared text removed. Your original files and regular search are kept."); Ok(())
                },
                "index"=> {
                    if !core::enabled(&e)? || !installed(&e) {return Err("Download and enable the model first.".into());}
                    *s.0.attempted.lock().unwrap_or_else(|e|e.into_inner())=core::pending_key(&e)?;
                    with_server(&e,&s,|server| {
                    progress(&s,"indexing","Preparing document text for meaning search on this computer…");
                    core::index(&e,&s.0.cancel,|text|server.embed(text,&s),|n|{let mut p=s.0.progress.lock().unwrap_or_else(|e|e.into_inner());p.processed=n.completed;p.total=n.total;p.indexed=n.indexed;p.skipped=n.skipped;})?;
                    progress(&s,"ready","Ready to search"); Ok(())
                    })
                },
                _=>unreachable!()
            }
        })();
        if result.is_ok() {s.0.progress.lock().unwrap_or_else(|e|e.into_inner()).finished=true;}
        e.0.revision.fetch_add(1, Ordering::Relaxed);
        if let Err(err)=result {if s.0.cancel.load(Ordering::Acquire) {s.0.attempted.lock().unwrap_or_else(|e|e.into_inner()).clear();} s.0.blocked.store(!s.0.cancel.load(Ordering::Acquire),Ordering::Release);progress(&s,if s.0.cancel.load(Ordering::Relaxed) {"cancelled"} else {"error"},&err);}
    });
    Ok(())
}
#[derive(Serialize)]
pub struct SearchResponse {result: QueryResult, matches: std::collections::HashMap<i64,core::MatchPassage>, semantic: bool, message: String}
#[tauri::command]
pub async fn semantic_query(state: State<'_,Engine>,sem: State<'_,Semantic>,query: Query, broader: Option<bool>) -> Result<SearchResponse> {
    let e=state.inner().clone(); let s=sem.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fallback=|query,message: &str| -> Result<SearchResponse> {Ok(SearchResponse{result:db::query(&db::connect(&e.0.db)?,query)?,matches:Default::default(),semantic:false,message:message.into()})};
        let text=query.terms.iter().filter(|t|!t.starts_with("ext:")).cloned().collect::<Vec<_>>().join(" ");
        if text.trim().is_empty() {return fallback(query,"");}
        if !supported() || !core::enabled(&e)? || !installed(&e) || core::counts(&e)?.0==0 {return fallback(query,"");}
        if s.0.blocked.load(Ordering::Acquire) {return fallback(query,"AI search needs attention. Showing existing search results; choose Retry in Settings.");}
        let assisted=digview_core::query_language::assist(&text);
        if assisted.is_none() && digview_core::query_language::urdu_wording(&text) {
            return fallback(query,"For this wording, try English or exact words from the document.");
        }
        let _guard=match acquire(&s) {Ok(x)=>x,Err(_)=>return fallback(query,"A semantic task is running. Showing existing search results.")};
        if s.0.blocked.load(Ordering::Acquire) {return fallback(query,"AI search needs attention. Showing existing search results; choose Retry in Settings.");}
        *s.0.progress.lock().unwrap_or_else(|e|e.into_inner())=Progress {task:"search".into(),..Default::default()};
        let result=with_server(&e,&s,|server| {
            progress(&s,"searching","Searching indexed text…");
            let embedding_text=assisted.as_deref().unwrap_or(&text);
            let vector=server.query_vector(format!("task: search result | query: {}",core::bounded_text(embedding_text,1200)),&s)?;
            check_cancel(&s)?;
            if !core::enabled(&e)? {return Err("Semantic search disabled.".into());}
            core::search_matches(&e, query.clone(), vector, &s.0.cancel, broader.unwrap_or(false))
        });
        match result {Ok(result)=>{progress(&s,"ready","Meaning search complete.");Ok(SearchResponse{result:result.result,matches:result.matches,semantic:true,message:if broader.unwrap_or(false) {"More related results · lower-confidence matches included".into()} else {"Exact matches and related passages".into()}})},Err(error)=>{search_failure(&s,&error);fallback(query,if s.0.cancel.load(Ordering::Acquire) {"AI search interrupted. Showing existing search results."} else {"Local model unavailable. Showing existing search results; choose Retry in Settings."})}}
    }).await.map_err(|e|e.to_string())?
}
pub fn shutdown(app: &tauri::AppHandle) {let s=app.state::<Semantic>();let _interrupt=interrupt(s.inner());s.0.stopping.store(true,Ordering::Release);stop(s.inner());s.0.runtime.lock().unwrap_or_else(|e|e.into_inner()).worker.take();}

/// Runs independently of the window so tray-mode indexing continues. Never downloads assets.
pub fn start_automation(e: Engine,s: Semantic) {
    std::thread::spawn(move || {
        while !s.0.stopping.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_secs(2));
            if s.0.stopping.load(Ordering::Acquire) {break;}
            // Inference holds this lock; idle cleanup must never make a search look busy.
            if let Ok(mut cache)=s.0.runtime.try_lock() {cache.expire(Instant::now());}
            let result=(|| -> Result<()> {
                if !supported() || !core::enabled(&e)? || !installed(&e) || !e.setup_status()?.completed {return Ok(());}
                if s.0.blocked.load(Ordering::Acquire) {return Ok(());}
                let scan=e.scan_status()?;
                if scan.suspended {
                    progress(&s,"paused","Background work paused by scanning settings");
                    let indexing=s.0.progress.lock().unwrap_or_else(|e|e.into_inner()).task=="index";
                    if s.0.busy.load(Ordering::Acquire) && indexing {let _interrupt=interrupt(&s);stop(&s);}
                    return Ok(());
                }
                if s.0.busy.load(Ordering::Acquire) || s.0.blocked.load(Ordering::Acquire) || e.job().running {return Ok(());}
                if db::roots(&db::connect(&e.0.db)?)?.is_empty() {progress(&s,"waiting","Choose a folder to start AI search");return Ok(());}
                let doc=e.document_index_status()?;
                let (_,pending)=core::counts(&e)?;
                let changed=core::pending_key(&e)?!=*s.0.attempted.lock().unwrap_or_else(|e|e.into_inner());
                match core::auto_step(true,e.job().running,doc.enabled,doc.pending,pending,changed) {
                    core::AutoStep::EnableText=>{e.set_document_index_enabled(true)?;},
                    core::AutoStep::ReadText=>{progress(&s,"documents","Reading document text…");if let Err(err)=e.start_document_index(false) {if !e.job().running {return Err(err);}}},
                    core::AutoStep::Embed=>{if let Err(err)=launch(e.clone(),s.clone(),"index".into()) {if !e.job().running && !s.0.busy.load(Ordering::Acquire) {return Err(err);}}},
                    core::AutoStep::Ready=>progress(&s,"ready",if doc.failed>0 {"Ready · some documents could not be read"} else {"Ready to search"}),
                    core::AutoStep::Wait=>{},
                }
                Ok(())
            })();
            if let Err(error)=result {s.0.blocked.store(true,Ordering::Release);progress(&s,"error",&error);}
        }
    });
}

#[tauri::command]
pub async fn matched_document(state:State<'_,Engine>,id:i64,digest:String)->Result<String>{
    let e=state.inner().clone();
    tauri::async_runtime::spawn_blocking(move||core::matched_text(&e,id,&digest)).await.map_err(|e|e.to_string())?
}

#[cfg(test)]
mod warm_worker_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Worker(Arc<AtomicUsize>);
    impl Drop for Worker {fn drop(&mut self) {self.0.fetch_add(1,Ordering::Relaxed);}}
    #[test]
    fn interrupt_cleanup_fences_new_queries_and_preserves_cancellation() {
        let s=Semantic::default();let active=acquire(&s).unwrap();
        let first=interrupt(&s);let second=interrupt(&s);drop(active);
        assert!(acquire(&s).is_err());assert!(s.0.cancel.load(Ordering::Acquire));
        drop(first);assert!(acquire(&s).is_err());
        drop(second);let _next=acquire(&s).unwrap();assert!(!s.0.cancel.load(Ordering::Acquire));
    }
    #[test]
    fn shutdown_does_not_allow_future_workers() {
        let s=Semantic::default();s.0.stopping.store(true,Ordering::Release);
        assert!(acquire(&s).is_err());
    }
    #[test]
    fn search_errors_remain_visible_but_do_not_override_pause_or_cancellation() {
        let s=Semantic::default();search_failure(&s,"Engine could not start");
        assert!(s.0.blocked.load(Ordering::Acquire));
        assert_eq!(s.0.progress.lock().unwrap().phase,"error");
        s.0.blocked.store(false,Ordering::Release);
        let pending=interrupt(&s);progress(&s,"paused","AI search paused");
        search_failure(&s,"Connection closed during pause");
        assert!(!s.0.blocked.load(Ordering::Acquire));assert_eq!(s.0.progress.lock().unwrap().phase,"paused");
        drop(pending);search_failure(&s,"Cancelled request");
        assert!(!s.0.blocked.load(Ordering::Acquire));assert_eq!(s.0.progress.lock().unwrap().phase,"cancelled");
    }
    #[test]
    fn reuses_worker_and_releases_it_only_after_idle_timeout() {
        let dropped=Arc::new(AtomicUsize::new(0));
        let mut cache=WarmWorker::default();let now=Instant::now();
        cache.get_or_start(now, |_|true, ||Ok(Worker(dropped.clone()))).unwrap();
        cache.get_or_start(now+Duration::from_secs(100), |_|true, ||panic!("must reuse warm worker")).unwrap();
        cache.expire(now+Duration::from_secs(219));assert!(cache.worker.is_some());
        cache.expire(now+Duration::from_secs(220));assert!(cache.worker.is_none());
        assert_eq!(dropped.load(Ordering::Relaxed),1);
    }
    #[test]
    fn dead_worker_is_dropped_before_restart_and_failed_start_is_retryable() {
        let dropped=Arc::new(AtomicUsize::new(0));let mut cache=WarmWorker::default();let now=Instant::now();
        cache.get_or_start(now, |_|true, ||Ok(Worker(dropped.clone()))).unwrap();
        assert!(cache.get_or_start(now, |_|false, ||{assert_eq!(dropped.load(Ordering::Relaxed),1);Err("offline".into())}).is_err());
        assert!(cache.worker.is_none());
        cache.get_or_start(now, |_|true, ||Ok(Worker(dropped.clone()))).unwrap();
    }
}
