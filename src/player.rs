//! High-level frame player that bundles animation, sizing, and rendering.
//!
//! [`FramePlayer`] is the main orchestrator that consumers embed. It owns
//! the loaded frames, animation controller, render config, and (on web)
//! the canvas cache. Consumers only need to supply platform-specific I/O
//! via [`FrameDataProvider`](crate::FrameDataProvider) and their UI
//! framework's timer / template glue.

use crate::{
    load_text_frames, render::RenderConfig, AnimationController, CFrameData, FontSizing, Frame,
    FrameDataProvider, FrameFile, LoadResult, ParseError,
};

/// A high-level frame player that bundles frame data, animation control,
/// font sizing, and (on web) a canvas cache into one struct.
pub struct FramePlayer {
    frames: Vec<Frame>,
    frame_files: Vec<FrameFile>,
    controller: AnimationController,
    config: RenderConfig,
    sizing: FontSizing,
    /// When `false`, [`render_frame`](Self::render_frame) always returns
    /// `Ok(false)` so the consumer draws the text fallback.  Automatically
    /// set to `true` by [`load_colors`](Self::load_colors) on success.
    color_ready: bool,
    #[cfg(feature = "web")]
    cache: crate::render::web::FrameCanvasCache,
}

impl FramePlayer {
    /// Create a new, empty player at the given FPS.
    pub fn new(fps: u32) -> Self {
        Self {
            frames: Vec::new(),
            frame_files: Vec::new(),
            controller: AnimationController::new(fps),
            config: RenderConfig::default(),
            sizing: FontSizing::default(),
            color_ready: false,
            #[cfg(feature = "web")]
            cache: crate::render::web::FrameCanvasCache::default(),
        }
    }

    // ── Construction & loading ───────────────────────────────────────

    /// Phase 1: load text frames from the given directory.
    ///
    /// After this returns successfully the player is ready for playback
    /// (text-only). Call [`frame_files`] to get the list needed for
    /// Phase 2 color loading.
    pub async fn load<P: FrameDataProvider>(&mut self, provider: &P, directory: &str) -> LoadResult<()> {
        let (frames, frame_files) = load_text_frames(provider, directory).await?;
        self.controller.set_frame_count(frames.len());
        #[cfg(feature = "web")]
        {
            self.cache.resize(frames.len());
            self.cache.invalidate_all();
        }
        self.frames = frames;
        self.frame_files = frame_files;
        self.color_ready = false;
        Ok(())
    }

    /// Replace the current contents with in-memory text frames.
    ///
    /// Useful when the caller already fetched / generated all frame text.
    pub fn set_text_frames(&mut self, contents: Vec<String>) {
        self.frames = contents.into_iter().map(Frame::text_only).collect();
        self.frame_files.clear();
        self.color_ready = false;
        self.controller.reset();
        self.controller.set_frame_count(self.frames.len());
        #[cfg(feature = "web")]
        {
            self.cache.resize(self.frames.len());
            self.cache.invalidate_all();
        }
    }

    /// Load colour data from one packed multi-frame blob.
    ///
    /// If no text frames are loaded yet, text content is reconstructed from
    /// the blob so playback still works.
    pub fn load_packed_colors(&mut self, data: &[u8]) -> Result<(), ParseError> {
        let blob = crate::parse_packed_cframes(data)?;
        let frame_count = blob.len();

        if self.frames.is_empty() {
            self.frames = (0..frame_count)
                .map(|index| {
                    let cframe = blob.decode_frame(index).expect("packed blob frame index should be valid");
                    let text = cframe.to_text();
                    Frame::with_color(text, cframe)
                })
                .collect();
            self.frame_files.clear();
            self.controller.reset();
            self.controller.set_frame_count(self.frames.len());
            #[cfg(feature = "web")]
            self.cache.resize(self.frames.len());
        } else if self.frames.len() != frame_count {
            return Err(ParseError::FrameCountMismatch {expected: self.frames.len(), actual: frame_count});
        } else {
            for index in 0..frame_count {
                let cframe = blob.decode_frame(index).expect("packed blob frame index should be valid");
                self.frames[index].cframe = Some(cframe);
            }
        }

        self.color_ready = true;
        #[cfg(feature = "web")]
        self.cache.invalidate_all();
        Ok(())
    }

    /// The frame file list returned by Phase 1, needed for
    /// [`load_color_frames`](crate::load_color_frames).
    pub fn frame_files(&self) -> &[FrameFile] {
        &self.frame_files
    }

    /// Phase 2 callback: store colour data for one frame.
    pub fn set_frame_color(&mut self, index: usize, cframe: CFrameData) {
        if index < self.frames.len() {
            self.frames[index].cframe = Some(cframe);
        }
    }

    /// Whether colour rendering is enabled.
    ///
    /// When `false`, [`render_frame`](Self::render_frame) always returns
    /// `Ok(false)` so the consumer draws the text fallback.
    /// [`load_colors`](Self::load_colors) sets this to `true` automatically
    /// on success; use [`set_color_ready`](Self::set_color_ready) for manual
    /// control.
    pub fn color_ready(&self) -> bool {
        self.color_ready
    }

    /// Manually enable or disable colour rendering.
    pub fn set_color_ready(&mut self, ready: bool) {
        self.color_ready = ready;
    }

    // ── Playback (delegates to AnimationController) ─────────────────

    /// Start or resume playback.
    pub fn play(&mut self) {
        self.controller.play();
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.controller.pause();
    }

    /// Toggle play / pause.
    pub fn toggle(&mut self) {
        self.controller.toggle();
    }

    /// Stop and reset to the start of the range.
    pub fn stop(&mut self) {
        self.controller.stop();
    }

    /// Advance one frame. Returns `true` if the frame changed.
    pub fn tick(&mut self) -> bool {
        self.controller.tick()
    }

    /// Step forward one frame (pauses playback).
    pub fn step_forward(&mut self) {
        self.controller.step_forward();
    }

    /// Step backward one frame (pauses playback).
    pub fn step_backward(&mut self) {
        self.controller.step_backward();
    }

    /// Seek to a percentage position (0.0 – 1.0) within the range.
    pub fn seek(&mut self, pct: f64) {
        self.controller.seek(pct);
    }

    /// Current position as a percentage (0.0 – 1.0).
    pub fn position(&self) -> f64 {
        self.controller.position()
    }

    /// Set the playback FPS.
    pub fn set_fps(&mut self, fps: u32) {
        self.controller.set_fps(fps);
    }

    /// Milliseconds between frames at the current FPS.
    pub fn interval_ms(&self) -> u32 {
        self.controller.interval_ms()
    }

    /// Whether the player is currently playing.
    pub fn is_playing(&self) -> bool {
        self.controller.is_playing()
    }

    /// Index of the current frame.
    pub fn current_frame(&self) -> usize {
        self.controller.current_frame()
    }

    /// Total number of loaded frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    // ── Content access ──────────────────────────────────────────────

    /// Text content of the current frame.
    pub fn current_text(&self) -> Option<&str> {
        self.frames
            .get(self.controller.current_frame())
            .map(|f| f.content.as_str())
    }

    /// Text content of an arbitrary frame.
    pub fn get_text(&self, index: usize) -> Option<&str> {
        self.frames.get(index).map(|f| f.content.as_str())
    }

    /// Whether a specific frame has colour data.
    pub fn has_color_at(&self, index: usize) -> bool {
        self.frames
            .get(index)
            .map(|f| f.has_color())
            .unwrap_or(false)
    }

    /// Whether any loaded frame has colour data.
    pub fn has_any_color(&self) -> bool {
        self.frames.iter().any(|f| f.has_color())
    }

    /// (cols, rows) from the first frame, or `None` if empty.
    pub fn dimensions(&self) -> Option<(usize, usize)> {
        self.frames.first().map(|f| f.dimensions())
    }

    // ── Sizing ──────────────────────────────────────────────────────

    /// Recalculate the font size to fit within the given container
    /// dimensions (in pixels). Invalidates the canvas cache on web.
    pub fn fit_to_container(&mut self, width: f64, height: f64) {
        if let Some((cols, rows)) = self.dimensions() {
            let font_size = self.sizing.calculate_font_size(cols, rows, width, height);
            self.config.font_size = font_size;
            self.config.sizing = self.sizing.clone();
            #[cfg(feature = "web")]
            self.cache.invalidate_all();
        }
    }

    /// Current font size in pixels.
    pub fn font_size(&self) -> f64 {
        self.config.font_size
    }

    /// Replace the render configuration.
    pub fn set_render_config(&mut self, config: RenderConfig) {
        self.sizing = config.sizing.clone();
        self.config = config;
        #[cfg(feature = "web")]
        self.cache.invalidate_all();
    }

    /// Borrow the current render config.
    pub fn render_config(&self) -> &RenderConfig {
        &self.config
    }

    /// CSS string suitable for styling a `<pre>` fallback element.
    ///
    /// Returns something like
    /// `"font-size: 12px; line-height: 13.32px; width: 480px; height: 266.4px;"`.
    pub fn font_size_css(&self) -> String {
        if let Some((cols, rows)) = self.dimensions() {
            let fs = self.config.font_size;
            let lh = self.config.line_height();
            let (w, h) = self.sizing.canvas_dimensions(cols, rows, fs);
            format!(
                "font-size: {:.2}px; line-height: {:.2}px; width: {:.2}px; height: {:.2}px;",
                fs, lh, w, h
            )
        } else {
            String::new()
        }
    }

    // ── Advanced access ─────────────────────────────────────────────

    /// Borrow the animation controller.
    pub fn controller(&self) -> &AnimationController {
        &self.controller
    }

    /// Mutably borrow the animation controller.
    pub fn controller_mut(&mut self) -> &mut AnimationController {
        &mut self.controller
    }

    /// Borrow the loaded frames.
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
}

// ── Web-only rendering helpers ──────────────────────────────────────

#[cfg(feature = "web")]
impl FramePlayer {
    /// Render the current frame to the given canvas.
    ///
    /// Tries the cache first, then renders + caches. Returns `Ok(true)` if
    /// a colour frame was drawn, `Ok(false)` if the consumer should fall
    /// back to the text content (via [`current_text`](Self::current_text)).
    pub fn render_current(&mut self, canvas: &web_sys::HtmlCanvasElement) -> Result<bool, String> {
        self.render_frame(self.controller.current_frame(), canvas)
    }

    /// Render an arbitrary frame to the given canvas.
    ///
    /// Returns `Ok(true)` if a colour frame was drawn, `Ok(false)` if the
    /// consumer should use the text fallback.
    pub fn render_frame(
        &mut self,
        index: usize,
        canvas: &web_sys::HtmlCanvasElement,
    ) -> Result<bool, String> {
        if !self.color_ready {
            return Ok(false);
        }

        let render_key = crate::render::web::current_render_key(&self.config);
        self.cache.invalidate_for_render_key(render_key);

        // Try cache first
        if crate::render::web::draw_frame_from_cache(canvas, &self.cache, index)? {
            return Ok(true);
        }

        // No cache hit – try to render the colour frame
        let cframe = match self.frames.get(index).and_then(|f| f.cframe.as_ref()) {
            Some(cf) => cf,
            None => return Ok(false),
        };

        // Render to an offscreen canvas, cache it, then blit to the target
        let offscreen = crate::render::web::render_to_offscreen_canvas(cframe, &self.config)?;
        self.cache.store(index, offscreen.clone());
        crate::render::web::draw_cached_canvas(canvas, &offscreen)?;
        Ok(true)
    }

    /// Pre-render one frame to the cache. Returns `true` if the frame was
    /// successfully cached (i.e. it has colour data and wasn't cached yet).
    pub fn pre_cache_frame(&mut self, index: usize) -> bool {
        let render_key = crate::render::web::current_render_key(&self.config);
        self.cache.invalidate_for_render_key(render_key);

        if self.cache.has(index) {
            return false;
        }
        let cframe = match self.frames.get(index).and_then(|f| f.cframe.as_ref()) {
            Some(cf) => cf,
            None => return false,
        };
        match crate::render::web::render_to_offscreen_canvas(cframe, &self.config) {
            Ok(offscreen) => {
                self.cache.store(index, offscreen);
                true
            }
            Err(_) => false,
        }
    }

    /// Advance one animation step and render the current frame to `canvas`.
    ///
    /// Handles the full render pipeline: colour frame from cache / render,
    /// or plain-text fallback.  No-op when the player is paused.
    pub fn tick_and_render(&mut self, canvas: &web_sys::HtmlCanvasElement) -> Result<(), String> {
        if !self.is_playing() {
            return Ok(());
        }
        self.tick();
        let idx = self.current_frame();
        if !self.render_current(canvas)? {
            if let Some(text) = self.frames.get(idx).map(|f| f.content.as_str()) {
                crate::render::web::render_text_to_canvas(canvas, text, &self.config)?;
            }
        }
        Ok(())
    }

    /// Phase 2: load colour data in the background.
    ///
    /// Since this runs concurrently with the animation timer (which also
    /// needs mutable access), the player must be wrapped in
    /// `Rc<RefCell<FramePlayer>>`.  Pass that handle here.
    ///
    /// Does **not** enable colour rendering automatically — call
    /// [`pre_cache_all`](Self::pre_cache_all) (which sets `color_ready`)
    /// for stutter-free first-loop playback, or
    /// [`set_color_ready(true)`](Self::set_color_ready) to start
    /// immediately (first loop will be slower while frames are cached
    /// on demand).
    pub async fn load_colors<P: FrameDataProvider>(
        player: &std::rc::Rc<std::cell::RefCell<Self>>,
        provider: &P,
    ) -> LoadResult<()> {
        let frame_files = player.borrow().frame_files().to_vec();
        let player_cb = player.clone();
        crate::load_color_frames(
            provider,
            &frame_files,
            |index, _total, cframe_opt| {
                if let Some(cframe) = cframe_opt {
                    player_cb.borrow_mut().set_frame_color(index, cframe);
                }
            },
            crate::yield_to_event_loop,
        )
        .await
    }

    /// Pre-render all colour frames to the canvas cache, then enable
    /// colour rendering.
    ///
    /// Call this after [`load_colors`](Self::load_colors) for smooth
    /// playback from the very first coloured loop.  Yields between
    /// frames so the text animation keeps running while caching.
    pub async fn pre_cache_all(
        player: &std::rc::Rc<std::cell::RefCell<Self>>,
    ) -> Result<(), String> {
        let count = player.borrow().frame_count();
        for i in 0..count {
            player.borrow_mut().pre_cache_frame(i);
            crate::yield_to_event_loop().await;
        }
        player.borrow_mut().color_ready = true;
        Ok(())
    }

    /// Borrow the canvas cache.
    pub fn cache(&self) -> &crate::render::web::FrameCanvasCache {
        &self.cache
    }

    /// Mutably borrow the canvas cache.
    pub fn cache_mut(&mut self) -> &mut crate::render::web::FrameCanvasCache {
        &mut self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_player() {
        let player = FramePlayer::new(24);
        assert_eq!(player.frame_count(), 0);
        assert!(!player.is_playing());
        assert_eq!(player.current_frame(), 0);
        assert!(player.current_text().is_none());
        assert!(player.dimensions().is_none());
    }

    #[test]
    fn test_player_with_frames() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![
            Frame::text_only("Hello\nWorld".into()),
            Frame::text_only("Frame 2".into()),
        ];
        player.controller.set_frame_count(2);

        assert_eq!(player.frame_count(), 2);
        assert_eq!(player.current_text(), Some("Hello\nWorld"));
        assert_eq!(player.get_text(1), Some("Frame 2"));
        assert_eq!(player.dimensions(), Some((5, 2)));
        assert!(!player.has_any_color());
    }

    #[test]
    fn test_player_playback() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![
            Frame::text_only("A".into()),
            Frame::text_only("B".into()),
            Frame::text_only("C".into()),
        ];
        player.controller.set_frame_count(3);

        player.play();
        assert!(player.is_playing());

        assert!(player.tick());
        assert_eq!(player.current_frame(), 1);

        player.pause();
        assert!(!player.is_playing());

        player.step_forward();
        assert_eq!(player.current_frame(), 2);

        player.step_backward();
        assert_eq!(player.current_frame(), 1);
    }

    #[test]
    fn test_player_seek() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![Frame::text_only("A".into()); 100];
        player.controller.set_frame_count(100);

        player.seek(0.5);
        assert_eq!(player.current_frame(), 50);
        assert!((player.position() - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_player_set_color() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![Frame::text_only("AB".into())];
        player.controller.set_frame_count(1);

        assert!(!player.has_color_at(0));

        let cframe = CFrameData::new(2, 1, vec![b'A', b'B'], vec![255, 0, 0, 0, 255, 0]);
        player.set_frame_color(0, cframe);

        assert!(player.has_color_at(0));
        assert!(player.has_any_color());
    }

    #[test]
    fn test_player_fit_to_container() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![Frame::text_only("ABCDEFGHIJ\n1234567890".into())];
        player.controller.set_frame_count(1);

        player.fit_to_container(800.0, 600.0);
        assert!(player.font_size() > 0.0);
    }

    #[test]
    fn test_player_font_size_css() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![Frame::text_only("ABC\nDEF".into())];
        player.controller.set_frame_count(1);

        player.config.font_size = 10.0;
        let css = player.font_size_css();
        assert!(css.contains("font-size:"));
        assert!(css.contains("line-height:"));
        assert!(css.contains("width:"));
        assert!(css.contains("height:"));
    }

    #[test]
    fn test_player_font_size_css_empty() {
        let player = FramePlayer::new(24);
        assert_eq!(player.font_size_css(), "");
    }

    #[test]
    fn test_player_advanced_access() {
        let mut player = FramePlayer::new(30);
        assert_eq!(player.controller().fps(), 30);

        player.controller_mut().set_fps(60);
        assert_eq!(player.controller().fps(), 60);

        player.set_fps(15);
        assert_eq!(player.controller().fps(), 15);

        assert_eq!(player.frames().len(), 0);
    }

    #[test]
    fn test_player_toggle_stop() {
        let mut player = FramePlayer::new(24);
        player.frames = vec![Frame::text_only("A".into()), Frame::text_only("B".into())];
        player.controller.set_frame_count(2);

        player.toggle();
        assert!(player.is_playing());

        player.toggle();
        assert!(!player.is_playing());

        player.play();
        player.tick();
        assert_eq!(player.current_frame(), 1);

        player.stop();
        assert!(!player.is_playing());
        assert_eq!(player.current_frame(), 0);
    }

    #[test]
    fn test_player_interval_ms() {
        let player = FramePlayer::new(24);
        assert_eq!(player.interval_ms(), 41);
    }
}
