//! # cascii-core-view
//!
//! Core frame display and animation library for ASCII art viewers.
//!
//! This crate provides platform-agnostic data structures and logic for:
//! - Loading and parsing ASCII frame data (text and color)
//! - Calculating optimal font sizes for display
//! - Controlling animation playback (speed, loop, stepping)
//! - Rendering frames to canvas (with optional web support)
//!
//! ## Features
//!
//! - `serde` - Enable serialization/deserialization for data structures
//! - `web` - Enable web/WASM canvas rendering support
//!
//! ## Example
//!
//! ```rust,ignore
//! use cascii_core_view::{Frame, CFrameData, AnimationController, FontSizing};
//!
//! // Parse a .cframe file
//! let cframe_data = cascii_core_view::parser::parse_cframe(&bytes)?;
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

mod animation;
mod data;
mod parser;
pub mod render;
mod sizing;

pub use animation::{AnimationController, AnimationState, LoopMode};
pub use data::{CFrameData, Frame, FrameFile};
pub use parser::{parse_cframe, parse_cframe_text, ParseError};
pub use render::{RenderConfig, RenderResult};
pub use sizing::FontSizing;

#[cfg(feature = "web")]
pub use render::web::render_to_canvas;
