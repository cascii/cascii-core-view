//! Animation controller for frame playback.

/// Loop mode for animation playback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopMode {
    /// Stop at the end of the animation
    Once,
    /// Loop back to start when reaching the end
    #[default]
    Loop,
}

/// Current state of the animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationState {
    /// Animation is stopped
    Stopped,
    /// Animation is playing
    Playing,
    /// Animation reached the end (only in LoopMode::Once)
    Finished,
}

/// Platform-agnostic animation controller for frame playback.
///
/// This controller manages the state of frame animation but does not
/// handle timing directly. The caller is responsible for calling `tick()`
/// at the appropriate rate (based on `interval_ms()`).
///
/// ## Example
///
/// ```rust
/// use cascii_core_view::{AnimationController, AnimationState};
///
/// let mut controller = AnimationController::new(24); // 24 FPS
/// controller.set_frame_count(100);
///
/// // Start playback
/// controller.play();
/// assert_eq!(controller.state(), AnimationState::Playing);
///
/// // Advance frames (call this from your timer)
/// for _ in 0..10 {
///     controller.tick();
/// }
/// assert_eq!(controller.current_frame(), 10);
///
/// // Pause
/// controller.pause();
/// assert_eq!(controller.state(), AnimationState::Stopped);
/// ```
#[derive(Clone, Debug)]
pub struct AnimationController {
    /// Current frame index
    current_frame: usize,
    /// Total number of frames
    frame_count: usize,
    /// Frames per second
    fps: u32,
    /// Current playback state
    state: AnimationState,
    /// Loop mode
    loop_mode: LoopMode,
    /// Range start (0.0 - 1.0)
    range_start: f64,
    /// Range end (0.0 - 1.0)
    range_end: f64,
}

impl AnimationController {
    /// Create a new animation controller with the given FPS.
    pub fn new(fps: u32) -> Self {
        Self {
            current_frame: 0,
            frame_count: 0,
            fps: fps.max(1),
            state: AnimationState::Stopped,
            loop_mode: LoopMode::Loop,
            range_start: 0.0,
            range_end: 1.0,
        }
    }

    /// Set the total number of frames.
    pub fn set_frame_count(&mut self, count: usize) {
        self.frame_count = count;
        // Clamp current frame to valid range
        if self.current_frame >= count && count > 0 {
            self.current_frame = count - 1;
        }
    }

    /// Get the total number of frames.
    #[inline]
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Set the playback FPS.
    pub fn set_fps(&mut self, fps: u32) {
        self.fps = fps.max(1);
    }

    /// Get the current FPS.
    #[inline]
    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// Get the interval in milliseconds between frames.
    ///
    /// Use this to configure your timer.
    #[inline]
    pub fn interval_ms(&self) -> u32 {
        (1000.0 / self.fps as f64).max(1.0) as u32
    }

    /// Set the loop mode.
    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
        // If we were finished and now set to loop, allow resuming
        if mode == LoopMode::Loop && self.state == AnimationState::Finished {
            self.state = AnimationState::Stopped;
        }
    }

    /// Get the current loop mode.
    #[inline]
    pub fn loop_mode(&self) -> LoopMode {
        self.loop_mode
    }

    /// Set the playback range (0.0 - 1.0).
    ///
    /// Frames outside this range will be skipped during playback.
    pub fn set_range(&mut self, start: f64, end: f64) {
        self.range_start = start.clamp(0.0, 1.0);
        self.range_end = end.clamp(0.0, 1.0).max(self.range_start + 0.01);

        // Clamp current frame to range
        let (start_frame, end_frame) = self.range_frames();
        if self.current_frame < start_frame || self.current_frame > end_frame {
            self.current_frame = start_frame;
        }
    }

    /// Get the current range as (start, end) in 0.0-1.0.
    #[inline]
    pub fn range(&self) -> (f64, f64) {
        (self.range_start, self.range_end)
    }

    /// Get the range as frame indices.
    pub fn range_frames(&self) -> (usize, usize) {
        if self.frame_count == 0 {
            return (0, 0);
        }
        let max_idx = self.frame_count.saturating_sub(1) as f64;
        let start = (self.range_start * max_idx).round() as usize;
        let end = (self.range_end * max_idx).round() as usize;
        (start, end)
    }

    /// Get the number of frames in the current range.
    pub fn range_frame_count(&self) -> usize {
        let (start, end) = self.range_frames();
        end.saturating_sub(start) + 1
    }

    /// Start or resume playback.
    pub fn play(&mut self) {
        if self.frame_count > 0 && self.state != AnimationState::Finished {
            self.state = AnimationState::Playing;
        }
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        if self.state == AnimationState::Playing {
            self.state = AnimationState::Stopped;
        }
    }

    /// Toggle play/pause.
    pub fn toggle(&mut self) {
        match self.state {
            AnimationState::Playing => self.pause(),
            AnimationState::Stopped => self.play(),
            AnimationState::Finished => {
                // Reset to start and play
                let (start, _) = self.range_frames();
                self.current_frame = start;
                self.state = AnimationState::Playing;
            }
        }
    }

    /// Stop playback and reset to the start of the range.
    pub fn stop(&mut self) {
        self.state = AnimationState::Stopped;
        let (start, _) = self.range_frames();
        self.current_frame = start;
    }

    /// Get the current playback state.
    #[inline]
    pub fn state(&self) -> AnimationState {
        self.state
    }

    /// Check if the animation is currently playing.
    #[inline]
    pub fn is_playing(&self) -> bool {
        self.state == AnimationState::Playing
    }

    /// Get the current frame index.
    #[inline]
    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    /// Set the current frame index.
    ///
    /// The frame will be clamped to the valid range.
    pub fn set_current_frame(&mut self, frame: usize) {
        if self.frame_count == 0 {
            self.current_frame = 0;
            return;
        }
        let (start, end) = self.range_frames();
        self.current_frame = frame.max(start).min(end);
    }

    /// Seek to a percentage position (0.0 - 1.0) within the current range.
    pub fn seek(&mut self, percentage: f64) {
        if self.frame_count == 0 {
            return;
        }
        let (start, end) = self.range_frames();
        let range_len = (end - start) as f64;
        let target = (start as f64 + percentage.clamp(0.0, 1.0) * range_len).round() as usize;
        self.current_frame = target.max(start).min(end);
    }

    /// Get the current position as a percentage (0.0 - 1.0) within the range.
    pub fn position(&self) -> f64 {
        let (start, end) = self.range_frames();
        if end <= start {
            return 0.0;
        }
        let range_len = (end - start) as f64;
        ((self.current_frame as f64) - (start as f64)) / range_len
    }

    /// Advance to the next frame.
    ///
    /// Call this method from your timer at the rate returned by `interval_ms()`.
    /// Returns true if the frame changed, false if animation stopped.
    pub fn tick(&mut self) -> bool {
        if self.state != AnimationState::Playing || self.frame_count == 0 {
            return false;
        }

        let (start, end) = self.range_frames();

        // Ensure we're within range
        if self.current_frame < start {
            self.current_frame = start;
            return true;
        }

        if self.current_frame >= end {
            match self.loop_mode {
                LoopMode::Loop => {
                    self.current_frame = start;
                    true
                }
                LoopMode::Once => {
                    self.state = AnimationState::Finished;
                    false
                }
            }
        } else {
            self.current_frame += 1;
            true
        }
    }

    /// Step forward one frame (manual stepping).
    ///
    /// Pauses playback and advances one frame, wrapping if at end.
    pub fn step_forward(&mut self) {
        if self.frame_count == 0 {
            return;
        }
        self.pause();

        let (start, end) = self.range_frames();
        self.current_frame = if self.current_frame >= end {
            start
        } else {
            self.current_frame + 1
        };
    }

    /// Step backward one frame (manual stepping).
    ///
    /// Pauses playback and goes back one frame, wrapping if at start.
    pub fn step_backward(&mut self) {
        if self.frame_count == 0 {
            return;
        }
        self.pause();

        let (start, end) = self.range_frames();
        self.current_frame = if self.current_frame <= start {
            end
        } else {
            self.current_frame - 1
        };
    }

    /// Reset the controller to initial state.
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.state = AnimationState::Stopped;
        self.range_start = 0.0;
        self.range_end = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_playback() {
        let mut ctrl = AnimationController::new(24);
        ctrl.set_frame_count(10);

        assert_eq!(ctrl.state(), AnimationState::Stopped);
        assert_eq!(ctrl.current_frame(), 0);

        ctrl.play();
        assert_eq!(ctrl.state(), AnimationState::Playing);

        // Advance 5 frames
        for _ in 0..5 {
            ctrl.tick();
        }
        assert_eq!(ctrl.current_frame(), 5);

        ctrl.pause();
        assert_eq!(ctrl.state(), AnimationState::Stopped);
        assert_eq!(ctrl.current_frame(), 5); // Should stay at 5
    }

    #[test]
    fn test_loop_mode() {
        let mut ctrl = AnimationController::new(24);
        ctrl.set_frame_count(5);
        ctrl.set_loop_mode(LoopMode::Loop);
        ctrl.play();

        // Advance to end and wrap
        for _ in 0..6 {
            ctrl.tick();
        }
        // Should have wrapped: 0 -> 1 -> 2 -> 3 -> 4 -> 0 -> 1
        assert_eq!(ctrl.current_frame(), 1);
        assert_eq!(ctrl.state(), AnimationState::Playing);
    }

    #[test]
    fn test_once_mode() {
        let mut ctrl = AnimationController::new(24);
        ctrl.set_frame_count(5);
        ctrl.set_loop_mode(LoopMode::Once);
        ctrl.play();

        // Advance to end
        for _ in 0..5 {
            ctrl.tick();
        }
        assert_eq!(ctrl.current_frame(), 4);
        assert_eq!(ctrl.state(), AnimationState::Finished);
    }

    #[test]
    fn test_range() {
        let mut ctrl = AnimationController::new(24);
        ctrl.set_frame_count(100);
        ctrl.set_range(0.25, 0.75); // Frames 25-75

        let (start, end) = ctrl.range_frames();
        assert_eq!(start, 25);
        assert_eq!(end, 74); // 0.75 * 99 ≈ 74

        ctrl.play();
        // Should start at range start
        assert_eq!(ctrl.current_frame(), 25);
    }

    #[test]
    fn test_step_forward_backward() {
        let mut ctrl = AnimationController::new(24);
        ctrl.set_frame_count(10);
        ctrl.set_current_frame(5);

        ctrl.step_forward();
        assert_eq!(ctrl.current_frame(), 6);
        assert_eq!(ctrl.state(), AnimationState::Stopped);

        ctrl.step_backward();
        assert_eq!(ctrl.current_frame(), 5);

        // Test wrap at end
        ctrl.set_current_frame(9);
        ctrl.step_forward();
        assert_eq!(ctrl.current_frame(), 0);

        // Test wrap at start
        ctrl.step_backward();
        assert_eq!(ctrl.current_frame(), 9);
    }

    #[test]
    fn test_seek() {
        let mut ctrl = AnimationController::new(24);
        ctrl.set_frame_count(100);

        ctrl.seek(0.5);
        assert_eq!(ctrl.current_frame(), 50);

        ctrl.seek(0.0);
        assert_eq!(ctrl.current_frame(), 0);

        ctrl.seek(1.0);
        assert_eq!(ctrl.current_frame(), 99);
    }

    #[test]
    fn test_interval_ms() {
        let ctrl = AnimationController::new(24);
        assert_eq!(ctrl.interval_ms(), 41); // 1000/24 ≈ 41.67

        let ctrl2 = AnimationController::new(60);
        assert_eq!(ctrl2.interval_ms(), 16); // 1000/60 ≈ 16.67
    }
}
