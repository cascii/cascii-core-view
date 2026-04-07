//! # cascii-core-view
//!
//! Core frame display and animation library for ASCII art viewers.
//!
//! This crate provides platform-agnostic data structures and logic for:
//! - Loading and parsing ASCII frame data (text and color)
//! - Parsing packed multi-frame color blobs for whole animations
//! - Calculating optimal font sizes for display
//! - Controlling animation playback (speed, loop, stepping)
//! - Rendering frames to canvas (with optional web support)
//! - High-level playback through [`FramePlayer`]
//!
//! ## Features
//!
//! - `serde` - Enable serialization/deserialization for data structures
//! - `web` - Enable web/WASM canvas rendering support
//!
//! ## Examples
//!
//! ```rust,ignore
//! use cascii_core_view::{AnimationController, FontSizing, Frame};
//!
//! // Parse a .cframe file
//! let cframe_data = cascii_core_view::parse_cframe(&bytes)?;
//!
//! // Create a frame
//! let frame = Frame {
//!     content: text_content,
//!     cframe: Some(cframe_data),
//! };
//!
//! // Calculate optimal font size
//! let font_size = FontSizing::calculate(80, 24, 800.0, 600.0);
//!
//! // Create animation controller
//! let mut controller = AnimationController::new(24); // 24 FPS
//! controller.set_frame_count(100);
//! controller.play();
//! ```
//!
//! ```rust,ignore
//! use cascii_core_view::{FontSizing, FramePlayer, RenderConfig};
//!
//! let mut player = FramePlayer::new(30);
//! player.set_text_frames(text_frames);
//! player.load_packed_colors(&packed_color_blob)?;
//!
//! let mut config = RenderConfig::new(12.0);
//! config.font_family = "Menlo, Monaco, 'Cascadia Mono', Consolas, monospace".into();
//! config.background_color = Some((0, 0, 0));
//! config.sizing = FontSizing {
//!     line_height_ratio: 14.0 / 12.0,
//!     ..FontSizing::default()
//! };
//! player.set_render_config(config);
//! player.play();
//! ```

mod animation;
mod color;
mod data;
mod details;
mod loader;
mod parser;
pub mod player;
pub mod render;
mod sizing;

pub use animation::{AnimationController, AnimationState, LoopMode};
pub use color::{parse_color, FrameColors};
pub use data::{CFrameData, Frame, FrameFile, PackedCFrameBlob};
pub use details::ProjectDetails;
pub use loader::{load_color_frames, load_text_frames, FrameDataProvider, FrameLoaderState, LoadResult, LoadingPhase, LoadingProgress};
pub use parser::{parse_cframe, parse_cframe_text, parse_packed_cframes, ParseError};
pub use player::FramePlayer;
pub use render::{RenderConfig, RenderResult};
pub use sizing::FontSizing;

#[cfg(feature = "web")]
pub use loader::yield_to_event_loop;
#[cfg(feature = "web")]
pub use render::web::render_to_canvas;
#[cfg(feature = "web")]
pub use render::web::{draw_cached_canvas, draw_frame_from_cache, render_text_to_canvas, render_to_offscreen_canvas, FrameCanvasCache};
