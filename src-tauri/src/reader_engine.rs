use kotoclip_core::{
    analysis::{AnalysisService, Request, ResourcePaths},
    library::{LibraryBook, LibraryBookSummary, ReaderLibrary},
    reader_state::{ReaderState, SavedSelection, WordState},
};
use serde_json::Value;
use std::{path::PathBuf, sync::{Arc, Mutex}};

pub struct ReaderEngine {
    pub analysis: Arc<AnalysisService>,
    library: ReaderLibrary,
    profile: Mutex<ReaderState>,
    active_session: Mutex<Option<String>>,
}

impl ReaderEngine {
    pub fn new(paths: ResourcePaths, library_root: PathBuf, data_root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&data_root).map_err(|error| error.to_string())?;
        Ok(Self {
            analysis: Arc::new(AnalysisService::new(paths)),
            library: ReaderLibrary::open(library_root).map_err(|error| error.to_string())?,
            profile: Mutex::new(ReaderState::open(data_root.join("reader-state.sqlite")).map_err(|error| error.to_string())?),
            active_session: Mutex::new(None),
        })
    }

    pub fn library_path(&self) -> String { self.library.root().to_string_lossy().into_owned() }
    pub fn books(&self) -> Result<Vec<LibraryBookSummary>, String> { self.library.list_books().map_err(|error| error.to_string()) }
    pub fn book_summary(&self, id: &str) -> Result<LibraryBookSummary, String> { self.library.book_summary(id).map_err(|error| error.to_string()) }
    pub fn backfill_resource_dimensions(&self) -> Result<(), String> { self.library.backfill_resource_dimensions().map_err(|error| error.to_string()) }
    pub fn import(&self, path: &str) -> Result<LibraryBook, String> { self.library.import_epub(path).map_err(|error| error.to_string()) }

    fn open_document(&self, document_id: Option<String>, text: String) -> Result<Value, String> {
        let mut active = self.active_session.lock().unwrap();
        if let Some(previous) = active.take() { self.analysis.dispatch(Request::CloseDocument { session_id: previous }); }
        let response = self.analysis.dispatch(Request::OpenDocument { document_id, text, policy: Default::default(), initial_offset: 0 });
        let value = response.result.ok_or_else(|| response.error.unwrap_or_else(|| "文档分析启动失败".into()))?;
        *active = value["update"]["session_id"].as_str().map(str::to_owned);
        Ok(value)
    }

    pub fn open_book(&self, id: &str) -> Result<LibraryBook, String> {
        self.close();
        self.library.open_book(id).map_err(|error| error.to_string())
    }

    pub fn open_text(&self, text: String) -> Result<Value, String> { self.open_document(None, text) }

    pub fn close(&self) {
        if let Some(previous) = self.active_session.lock().unwrap().take() {
            self.analysis.dispatch(Request::CloseDocument { session_id: previous });
        }
    }

    pub fn progress(&self, id: &str, offset: usize, total: usize, chapter: Option<&str>, seconds: u64) -> Result<LibraryBookSummary, String> {
        self.library.update_progress(id, offset, total, chapter, seconds).map_err(|error| error.to_string())
    }
    pub fn organize(&self, id: &str, color: Option<&str>, tags: &[String]) -> Result<LibraryBookSummary, String> {
        self.library.update_organization(id, color, tags).map_err(|error| error.to_string())
    }
    pub fn reset(&self, id: &str) -> Result<LibraryBookSummary, String> { self.library.reset_progress(id).map_err(|error| error.to_string()) }
    pub fn remove(&self, id: &str) -> Result<bool, String> { self.library.remove_book(id).map_err(|error| error.to_string()) }
    pub fn word(&self, base: &str, reading: &str) -> Result<WordState, String> {
        self.profile.lock().unwrap().word(base, reading).map_err(|error| error.to_string())
    }
    pub fn mark(&self, base: &str, reading: &str, known: bool) -> Result<WordState, String> {
        let profile = self.profile.lock().unwrap();
        profile.mark(base, reading, known).map_err(|error| error.to_string())?;
        profile.word(base, reading).map_err(|error| error.to_string())
    }
    pub fn expose(&self, base: &str, reading: &str) -> Result<(), String> {
        self.profile.lock().unwrap().expose(base, reading).map_err(|error| error.to_string())
    }
    pub fn selections(&self, book: &str, version: &str) -> Result<Vec<SavedSelection>, String> {
        self.profile.lock().unwrap().selections(book, version).map_err(|error| error.to_string())
    }
    pub fn save_selection(&self, selection: &SavedSelection) -> Result<(), String> {
        self.profile.lock().unwrap().save_selection(selection).map_err(|error| error.to_string())
    }
    pub fn delete_selection(&self, book: &str, version: &str, start: usize, end: usize) -> Result<(), String> {
        self.profile.lock().unwrap().delete_selection(book, version, start, end).map_err(|error| error.to_string())
    }
    pub fn clear_selections(&self, book: &str, version: &str) -> Result<(), String> {
        self.profile.lock().unwrap().clear_selections(book, version).map_err(|error| error.to_string())
    }
}
