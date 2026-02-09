//! Frame loading utilities and state management.
//!
//! This module provides abstractions for loading frames in two phases:
//! 1. Text frames (fast) - enables immediate playback
//! 2. Color data (background) - progressive enhancement

use crate::{CFrameData, Frame, FrameFile};

/// Loading phase indicator
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadingPhase {
    /// Not loading anything
    Idle,
    /// Loading text frames (Phase 1)
    LoadingText,
    /// Loading color data in background (Phase 2)
    LoadingColors,
    /// All loading complete
    Complete,
}

/// Progress information for frame loading
#[derive(Clone, Debug, Default)]
pub struct LoadingProgress {
    /// Number of text frames loaded
    pub text_loaded: usize,
    /// Total number of text frames to load
    pub text_total: usize,
    /// Number of color frames loaded
    pub color_loaded: usize,
    /// Total number of color frames to load
    pub color_total: usize,
}

impl LoadingProgress {
    /// Create a new loading progress tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset progress for a new loading session
    pub fn reset(&mut self, total: usize) {
        self.text_loaded = 0;
        self.text_total = total;
        self.color_loaded = 0;
        self.color_total = total;
    }

    /// Get text loading percentage (0-100)
    pub fn text_percent(&self) -> u8 {
        if self.text_total == 0 {
            0
        } else {
            ((self.text_loaded as f32 / self.text_total as f32) * 100.0) as u8
        }
    }

    /// Get color loading percentage (0-100)
    pub fn color_percent(&self) -> u8 {
        if self.color_total == 0 {
            0
        } else {
            ((self.color_loaded as f32 / self.color_total as f32) * 100.0) as u8
        }
    }

    /// Check if text loading is complete
    pub fn text_complete(&self) -> bool {
        self.text_total > 0 && self.text_loaded >= self.text_total
    }

    /// Check if color loading is complete
    pub fn color_complete(&self) -> bool {
        self.color_total > 0 && self.color_loaded >= self.color_total
    }

    /// Format text loading message
    pub fn text_message(&self) -> String {
        if self.text_total > 0 {
            format!(
                "Loading frames... {} / {} ({}%)",
                self.text_loaded,
                self.text_total,
                self.text_percent()
            )
        } else {
            "Loading frames...".to_string()
        }
    }

    /// Format color loading message (returns None if not loading colors)
    pub fn color_message(&self) -> Option<String> {
        if self.color_total > 0 && !self.color_complete() {
            Some(format!("Loading colors: {}%", self.color_percent()))
        } else {
            None
        }
    }
}

/// State for managing frame loading
#[derive(Clone, Debug)]
pub struct FrameLoaderState {
    /// Current loading phase
    pub phase: LoadingPhase,
    /// Loading progress
    pub progress: LoadingProgress,
    /// Loaded frames
    pub frames: Vec<Frame>,
    /// Frame file paths (for color loading phase)
    pub frame_paths: Vec<String>,
    /// Error message if loading failed
    pub error: Option<String>,
}

impl Default for FrameLoaderState {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameLoaderState {
    /// Create a new loader state
    pub fn new() -> Self {
        Self {
            phase: LoadingPhase::Idle,
            progress: LoadingProgress::new(),
            frames: Vec::new(),
            frame_paths: Vec::new(),
            error: None,
        }
    }

    /// Reset state for a new loading session
    pub fn reset(&mut self) {
        self.phase = LoadingPhase::Idle;
        self.progress = LoadingProgress::new();
        self.frames.clear();
        self.frame_paths.clear();
        self.error = None;
    }

    /// Start loading with the given frame files
    pub fn start_loading(&mut self, frame_files: &[FrameFile]) {
        self.reset();
        self.phase = LoadingPhase::LoadingText;
        self.progress.reset(frame_files.len());
        self.frame_paths = frame_files.iter().map(|f| f.path.clone()).collect();
    }

    /// Add a text-only frame (Phase 1)
    pub fn add_text_frame(&mut self, content: String) {
        self.frames.push(Frame::text_only(content));
        self.progress.text_loaded += 1;
    }

    /// Finish text loading phase and start color loading
    pub fn finish_text_loading(&mut self) {
        if self.frames.is_empty() {
            self.error = Some("No frames found".to_string());
            self.phase = LoadingPhase::Idle;
        } else {
            self.phase = LoadingPhase::LoadingColors;
        }
    }

    /// Update a frame with color data (Phase 2)
    pub fn set_frame_color(&mut self, index: usize, cframe: CFrameData) {
        if index < self.frames.len() {
            self.frames[index].cframe = Some(cframe);
        }
        self.progress.color_loaded += 1;

        // Check if all colors are loaded
        if self.progress.color_complete() {
            self.phase = LoadingPhase::Complete;
        }
    }

    /// Skip color data for a frame (no .cframe file exists)
    pub fn skip_frame_color(&mut self) {
        self.progress.color_loaded += 1;

        if self.progress.color_complete() {
            self.phase = LoadingPhase::Complete;
        }
    }

    /// Set an error and stop loading
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.phase = LoadingPhase::Idle;
    }

    /// Check if playback can start (text loading complete)
    pub fn can_play(&self) -> bool {
        !self.frames.is_empty()
            && (self.phase == LoadingPhase::LoadingColors || self.phase == LoadingPhase::Complete)
    }

    /// Check if any frame has color data
    pub fn has_any_color(&self) -> bool {
        self.frames.iter().any(|f| f.has_color())
    }

    /// Get the frame at the given index
    pub fn get_frame(&self, index: usize) -> Option<&Frame> {
        self.frames.get(index)
    }

    /// Get the number of loaded frames
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get the path for a frame (for loading color data)
    pub fn get_frame_path(&self, index: usize) -> Option<&str> {
        self.frame_paths.get(index).map(|s| s.as_str())
    }
}

/// Result type for frame loading operations
pub type LoadResult<T> = Result<T, String>;

/// Trait for async frame data providers.
///
/// Implement this trait to provide frame data from your specific I/O mechanism
/// (Tauri commands, fetch API, filesystem, etc.)
///
/// No `Send` bounds — works in both native and WASM (single-threaded) contexts.
pub trait FrameDataProvider {
    /// Get the list of frame files in a directory
    fn get_frame_files(&self, directory: &str) -> impl std::future::Future<Output = LoadResult<Vec<FrameFile>>>;

    /// Read frame text content
    fn read_frame_text(&self, path: &str) -> impl std::future::Future<Output = LoadResult<String>>;

    /// Read raw .cframe bytes for the given text frame path.
    ///
    /// Returns `Ok(None)` if no .cframe file exists for this frame.
    /// The caller (orchestrator) handles parsing via `parse_cframe`.
    fn read_cframe_bytes(&self, txt_path: &str) -> impl std::future::Future<Output = LoadResult<Option<Vec<u8>>>>;
}

/// Phase 1: load all text frames sequentially, return them along with the
/// file list (needed for Phase 2 color loading).
pub async fn load_text_frames<P: FrameDataProvider>(provider: &P, directory: &str) -> LoadResult<(Vec<Frame>, Vec<FrameFile>)> {
    let frame_files = provider.get_frame_files(directory).await?;

    if frame_files.is_empty() {
        return Err("No frames found in directory".to_string());
    }

    let mut frames = Vec::with_capacity(frame_files.len());
    for frame_file in &frame_files {
        let content = provider.read_frame_text(&frame_file.path).await?;
        frames.push(Frame::text_only(content));
    }

    Ok((frames, frame_files))
}

/// Phase 2: load color data in the background.
///
/// For each frame file, reads raw .cframe bytes, parses them via
/// `parse_cframe`, then calls `on_frame(index, total, Option<CFrameData>)`
/// so the caller can store the result. Calls `yield_fn()` before and after
/// each frame to keep the UI responsive (important in single-threaded WASM
/// contexts).
pub async fn load_color_frames<P, F, Y, YFut>(provider: &P, frame_files: &[FrameFile], on_frame: F, yield_fn: Y) -> LoadResult<()> where P: FrameDataProvider, F: Fn(usize, usize, Option<CFrameData>), Y: Fn() -> YFut, YFut: std::future::Future<Output = ()> {
    let total = frame_files.len();
    for (i, frame_file) in frame_files.iter().enumerate() {
        // Let animation/input callbacks run before potentially heavy read+parse work.
        yield_fn().await;

        let cframe = match provider.read_cframe_bytes(&frame_file.path).await? {
            Some(bytes) => crate::parse_cframe(&bytes).ok(),
            None => None,
        };
        on_frame(i, total, cframe);

        // Yield again after storing the decoded frame.
        yield_fn().await;
    }
    Ok(())
}

/// Yield control back to the browser event loop.
///
/// Useful in long-running WASM loops to keep UI responsive while background
/// loading or pre-rendering progresses.
#[cfg(feature = "web")]
pub async fn yield_to_event_loop() {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0);
        } else {
            let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_progress() {
        let mut progress = LoadingProgress::new();
        progress.reset(10);

        assert_eq!(progress.text_percent(), 0);
        assert!(!progress.text_complete());

        progress.text_loaded = 5;
        assert_eq!(progress.text_percent(), 50);

        progress.text_loaded = 10;
        assert!(progress.text_complete());
        assert_eq!(progress.text_percent(), 100);
    }

    #[test]
    fn test_loader_state_phases() {
        let mut state = FrameLoaderState::new();
        assert_eq!(state.phase, LoadingPhase::Idle);
        assert!(!state.can_play());

        let files = vec![
            FrameFile::new("frame_0001.txt".into(), "frame_0001.txt".into(), 1),
            FrameFile::new("frame_0002.txt".into(), "frame_0002.txt".into(), 2),
        ];

        state.start_loading(&files);
        assert_eq!(state.phase, LoadingPhase::LoadingText);
        assert!(!state.can_play());

        state.add_text_frame("Frame 1".into());
        state.add_text_frame("Frame 2".into());
        state.finish_text_loading();

        assert_eq!(state.phase, LoadingPhase::LoadingColors);
        assert!(state.can_play());
        assert_eq!(state.frame_count(), 2);

        state.skip_frame_color();
        state.skip_frame_color();

        assert_eq!(state.phase, LoadingPhase::Complete);
    }

    #[test]
    fn test_loader_state_with_colors() {
        let mut state = FrameLoaderState::new();
        let files = vec![FrameFile::new("frame_0001.txt".into(), "frame_0001.txt".into(), 1)];

        state.start_loading(&files);
        state.add_text_frame("Frame 1".into());
        state.finish_text_loading();

        assert!(!state.has_any_color());

        let cframe = CFrameData::new(2, 1, vec![b'A', b'B'], vec![255, 0, 0, 0, 255, 0]);
        state.set_frame_color(0, cframe);

        assert!(state.has_any_color());
        assert!(state.frames[0].has_color());
        assert_eq!(state.phase, LoadingPhase::Complete);
    }
}
