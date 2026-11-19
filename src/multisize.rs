//! Versioned CSMV assets and size switching with one animation controller.
use crate::{parse_cframe, AnimationController, Frame, RenderConfig};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::ops::Range;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SizeVariant {
    pub columns: u32,
    pub rows: Vec<u32>,
}

/// Logical ASCII grid dimensions for one frame at one generated size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSize {
    pub columns: u32,
    pub rows: u32,
}

/// How a requested column count is matched against generated variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeMatch {
    /// Require the exact generated width.
    Exact,
    /// Choose the nearest width, preferring the smaller width on a tie.
    Nearest,
    /// Choose the largest width no greater than requested; fail if none exists.
    Floor,
    /// Choose the smallest width no less than requested; fail if none exists.
    Ceil,
}

/// A successful size request. Unchanged boundary requests are successful no-ops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeChange {
    pub previous: FrameSize,
    pub current: FrameSize,
}

impl SizeChange {
    /// Whether the caller needs to redraw for a different logical grid.
    pub fn changed(&self) -> bool {self.previous != self.current}
}

/// A snapshot suitable for size pickers and smaller/larger button state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeState {
    pub current: FrameSize,
    pub available: Vec<FrameSize>,
    pub default_columns: u32,
    pub can_smaller: bool,
    pub can_larger: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiSizeManifest {
    pub format: String,
    pub schema_version: u32,
    pub frame_count: u32,
    pub fps: u32,
    pub default_columns: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_map: Option<Vec<u32>>,
    pub font_ratio: f32,
    pub luminance: u8,
    pub ascii_chars: String,
    pub text: bool,
    pub color: bool,
    pub audio: Option<String>,
    pub outputs: Vec<String>,
    pub variants: Vec<SizeVariant>,
}

impl MultiSizeManifest {
    /// Largest generated grid fitting both limits at this point in the timeline.
    pub fn fitting_columns(&self, frame: usize, columns: u32, rows: u32) -> Option<u32> {
        self.variants.iter().rev().find(|v| v.columns <= columns && v.rows.get(frame).is_some_and(|height| *height <= rows)).map(|v| v.columns)
    }

    /// Nearest stored grid, preferring the smaller of two equally close widths.
    pub fn nearest_columns(&self, requested: f64) -> Option<u32> {
        self.variants.iter().min_by(|left, right| {
            (f64::from(left.columns) - requested).abs().total_cmp(&(f64::from(right.columns) - requested).abs())
        }).map(|variant| variant.columns)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != "cascii-multisize" || !(1..=2).contains(&self.schema_version) || self.frame_count == 0 || self.frame_count > 1_000_000 || self.fps == 0 || self.fps > 1000 {
            return Err("Invalid multi-size format or timing".into());
        }
        if !self.font_ratio.is_finite() || self.font_ratio <= 0.0 || !(self.text || self.color) || self.ascii_chars.is_empty() || !self.ascii_chars.bytes().all(|byte| (32..=126).contains(&byte)) || self.audio.as_deref().is_some_and(|path| path != "audio.mp3") {
            return Err("Invalid multi-size rendering metadata".into());
        }
        if self.variants.is_empty() || self.variants.len() > 256 {
            return Err("Invalid variant count".into());
        }
        if self.column_map.is_some() != (self.schema_version == 2) || self.column_map.as_ref().is_some_and(|map| map.len() != self.frame_count as usize || map.iter().any(|c| !self.variants.iter().any(|v| v.columns == *c))) {
            return Err("Invalid per-frame column map".into());
        }
        let mut outputs = self.outputs.clone();
        outputs.sort();
        outputs.dedup();
        if outputs.is_empty() || outputs.len() != self.outputs.len() || outputs.iter().any(|path| !["variants/", "frames.bin", "frames.json", "frames.csv"].contains(&path.as_str())) {
            return Err("Invalid asset output locations".into());
        }
        let mut previous = 0;
        for variant in &self.variants {
            if variant.columns <= previous || variant.columns > 4096 || variant.rows.len() != self.frame_count as usize || variant.rows.iter().any(|&rows| rows == 0 || u64::from(rows) * u64::from(variant.columns) > 16_777_216) {
                return Err("Invalid variant dimensions or ordering".into());
            }
            previous = variant.columns;
        }
        if !self.variants.iter().any(|variant| variant.columns == self.default_columns) {
            return Err("Default size is missing".into());
        }
        Ok(())
    }
}

struct Entry {
    offset: usize,
    length: usize,
}

/// Random-access input. File and HTTP clients can fetch just the index and the selected payload.
pub trait FrameBytes {
    fn read_range(&self, range: Range<usize>) -> Result<Cow<'_, [u8]>, String>;
}

impl<T: AsRef<[u8]>> FrameBytes for T {
    fn read_range(&self, range: Range<usize>) -> Result<Cow<'_, [u8]>, String> {
        self.as_ref().get(range).map(Cow::Borrowed).ok_or_else(|| "Truncated asset range".into())
    }
}

pub struct IndexedMultiSizeAsset<B> {
    pub manifest: MultiSizeManifest,
    source: B,
    entries: Vec<Entry>,
}

pub type MultiSizeAsset<'a> = IndexedMultiSizeAsset<&'a [u8]>;

fn integer(data: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(data.get(offset..offset + 4).ok_or("Truncated asset integer")?.try_into().map_err(|_| "Invalid asset integer")?))
}

impl<'a> MultiSizeAsset<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, String> {
        Self::open(data, data.len())
    }
}

impl<B: FrameBytes> IndexedMultiSizeAsset<B> {
    pub fn open(source: B, length: usize) -> Result<Self, String> {
        let header = source.read_range(0..16)?;
        let version = integer(&header, 4)?;
        if header.get(..4) != Some(b"CSMV") || !(1..=2).contains(&version) {
            return Err("Unsupported multi-size binary header".into());
        }
        let metadata_len = integer(&header, 8)? as usize;
        let count = integer(&header, 12)? as usize;
        if metadata_len == 0 || metadata_len > 16 * 1024 * 1024 || count == 0 || count > 2_000_000 {
            return Err("Excessive asset metadata or index".into());
        }
        let metadata_end = 16 + metadata_len;
        let payload_start = metadata_end + count * 24;
        if payload_start > length {return Err("Truncated asset index".into());}
        let metadata = source.read_range(16..metadata_end)?;
        let manifest: MultiSizeManifest = serde_json::from_slice(&metadata).map_err(|error| error.to_string())?;
        manifest.validate()?;
        if manifest.schema_version != version {return Err("Binary header/schema version mismatch".into());}
        if count != manifest.frame_count as usize * manifest.variants.len() {return Err("Asset index count mismatch".into());}
        let data = source.read_range(metadata_end..payload_start)?;
        if data.len() != count * 24 {return Err("Truncated frame index".into());}
        let mut entries = Vec::with_capacity(count);
        let length_of_payload = length - payload_start;
        let mut expected = 0usize;
        for i in 0..count {
            let pos = i * 24;
            let offset = u64::from_le_bytes(data[pos + 8..pos + 16].try_into().map_err(|_| "Invalid offset")?);
            let offset = usize::try_from(offset).map_err(|_| "Offset exceeds platform size")?;
            let length = integer(&data, pos + 16)? as usize;
            if integer(&data, pos)? != manifest.variants[i % manifest.variants.len()].columns || integer(&data, pos + 4)? as usize != i / manifest.variants.len() || integer(&data, pos + 20)? != u32::from(manifest.color) || offset != expected || length == 0 {
                return Err("Invalid asset index entry".into());
            }
            let variant = &manifest.variants[i % manifest.variants.len()];
            let rows = variant.rows[i / manifest.variants.len()] as usize;
            let cells = variant.columns as usize * rows;
            let maximum = if manifest.color {9 + cells * 7} else {cells + rows};
            if length > maximum {return Err("Frame payload exceeds declared dimensions".into());}
            expected = offset.checked_add(length).ok_or("Payload length overflow")?;
            if expected > length_of_payload {return Err("Truncated frame payload".into());}
            entries.push(Entry {offset: payload_start + offset, length});
        }
        if expected != length_of_payload {return Err("Trailing asset data".into());}
        drop((data, metadata, header));
        Ok(Self {manifest, source, entries})
    }

    pub fn frame(&self, columns: u32, index: usize) -> Result<Frame, String> {
        let variant_index = self.manifest.variants.iter().position(|variant| variant.columns == columns).ok_or("Requested size is missing")?;
        let variant = &self.manifest.variants[variant_index];
        let rows = *variant.rows.get(index).ok_or("Frame index is out of range")?;
        let entry = &self.entries[index * self.manifest.variants.len() + variant_index];
        let payload = self.source.read_range(entry.offset..entry.offset + entry.length)?;
        let bytes = payload.as_ref();
        let frame = if self.manifest.color {
            let cells = columns as usize * rows as usize;
            let base = 8 + cells * 4;
            if bytes.len() < base || integer(bytes, 0)? != columns || integer(bytes, 4)? != rows || !(bytes.len() == base || (bytes.len() == base + 1 + cells * 3 && bytes[base] == 1)) {
                return Err("Cframe metadata or extension mismatch".into());
            }
            let cframe = parse_cframe(bytes).map_err(|error| error.to_string())?;
            Frame::with_color(cframe.to_text(), cframe)
        } else {
            Frame::text_only(std::str::from_utf8(bytes).map_err(|error| error.to_string())?.to_string())
        };
        let text = frame.content.strip_suffix('\n').unwrap_or(&frame.content);
        if text.split('\n').count() != rows as usize || text.split('\n').any(|line| line.len() != columns as usize || !line.bytes().all(|byte| (32..=126).contains(&byte))) {
            return Err("Frame glyphs or dimensions mismatch".into());
        }
        Ok(frame)
    }
}

/// Selection changes never replace the controller, its speed, range or state.
pub struct MultiSizePlayer<'a> {
    pub animation: AnimationController,
    asset: MultiSizeAsset<'a>,
    columns: u32,
    follow_map: bool,
    cache: VecDeque<(u32, usize, Frame)>,
    capacity: usize,
    byte_capacity: usize,
    cached_bytes: usize,
}

fn frame_bytes(frame: &Frame) -> usize {
    frame.content.len() + frame.cframe.as_ref().map_or(0, |frame| frame.chars.len() + frame.rgb.len() + frame.bg_rgb.as_ref().map_or(0, Vec::len))
}

impl<'a> MultiSizePlayer<'a> {
    pub fn new(asset: MultiSizeAsset<'a>) -> Self {
        let mut animation = AnimationController::new(asset.manifest.fps);
        animation.set_frame_count(asset.manifest.frame_count as usize);
        let columns = asset.manifest.default_columns;
        Self {animation, asset, columns, follow_map: true, cache: VecDeque::new(), capacity: 256, byte_capacity: 32 * 1024 * 1024, cached_bytes: 0}
    }

    pub fn selected_columns(&self) -> u32 {
        if self.follow_map {self.asset.manifest.column_map.as_ref().and_then(|m| m.get(self.animation.current_frame())).copied().unwrap_or(self.asset.manifest.default_columns)} else {self.columns}
    }
    /// Resume the saved per-frame selection after a manual size override.
    pub fn follow_column_map(&mut self) {self.follow_map = true;}
    /// Change a live selection without changing the animation clock.
    pub fn set_column_map(&mut self, map: Vec<u32>) -> Result<(), String> {
        if map.len() != self.asset.manifest.frame_count as usize || map.iter().any(|c| !self.asset.manifest.variants.iter().any(|v| v.columns == *c)) {return Err("Invalid per-frame column map".into());}
        self.asset.manifest.schema_version = 2; self.asset.manifest.column_map = Some(map); self.follow_map = true; Ok(())
    }
    pub fn cached_frame_count(&self) -> usize {self.cache.len()}
    pub fn cached_bytes(&self) -> usize {self.cached_bytes}
    pub fn manifest(&self) -> &MultiSizeManifest {&self.asset.manifest}

    /// Current grid dimensions, including this frame's actual row count.
    pub fn current_size(&self) -> Result<FrameSize, String> {
        self.dimensions_for(self.selected_columns())
    }

    /// Generated sizes in ascending column order at the current playback frame.
    pub fn available_sizes(&self) -> Result<Vec<FrameSize>, String> {
        let index = self.animation.current_frame();
        self.asset.manifest.variants.iter().map(|variant| {
            let rows = *variant.rows.get(index).ok_or("Frame index is out of range")?;
            Ok(FrameSize {columns: variant.columns, rows})
        }).collect()
    }

    /// Read-only UI state; this neither decodes frames nor changes playback.
    pub fn size_state(&self) -> Result<SizeState, String> {
        Ok(SizeState {
            current: self.current_size()?,
            available: self.available_sizes()?,
            default_columns: self.asset.manifest.default_columns,
            can_smaller: self.can_smaller(),
            can_larger: self.can_larger(),
        })
    }

    pub fn can_smaller(&self) -> bool {
        self.asset.manifest.variants.iter().any(|variant| variant.columns < self.selected_columns())
    }

    pub fn can_larger(&self) -> bool {
        self.asset.manifest.variants.iter().any(|variant| variant.columns > self.selected_columns())
    }

    /// Select the previous generated width, or stay at the smallest width.
    pub fn smaller(&mut self) -> Result<SizeChange, String> {
        let columns = self.asset.manifest.variants.iter().rev().find(|variant| variant.columns < self.selected_columns()).map_or(self.selected_columns(), |variant| variant.columns);
        self.change_size(columns)
    }

    /// Select the next generated width, or stay at the largest width.
    pub fn larger(&mut self) -> Result<SizeChange, String> {
        let columns = self.asset.manifest.variants.iter().find(|variant| variant.columns > self.selected_columns()).map_or(self.selected_columns(), |variant| variant.columns);
        self.change_size(columns)
    }

    /// Select an exact generated column count, retaining its source aspect ratio.
    pub fn set_size(&mut self, columns: u32) -> Result<SizeChange, String> {
        self.resize_to(columns, SizeMatch::Exact)
    }

    /// Select an exact generated grid, for example an item from available_sizes.
    /// Rows must match this frame; this never stretches or resamples ASCII cells.
    pub fn set_dimensions(&mut self, requested: FrameSize) -> Result<SizeChange, String> {
        if self.dimensions_for(requested.columns)? != requested {
            return Err("Requested rows do not match the generated size at this frame".into());
        }
        self.change_size(requested.columns)
    }

    /// Select a generated width using an explicit matching policy.
    pub fn resize_to(&mut self, requested: u32, matching: SizeMatch) -> Result<SizeChange, String> {
        if requested == 0 {return Err("Requested columns must be positive".into());}
        let variants = &self.asset.manifest.variants;
        let selected = match matching {
            SizeMatch::Exact => variants.iter().find(|variant| variant.columns == requested),
            SizeMatch::Nearest => variants.iter().min_by_key(|variant| (variant.columns.abs_diff(requested), variant.columns)),
            SizeMatch::Floor => variants.iter().rev().find(|variant| variant.columns <= requested),
            SizeMatch::Ceil => variants.iter().find(|variant| variant.columns >= requested),
        }.ok_or("No generated size matches the request")?.columns;
        self.change_size(selected)
    }

    /// Restore the asset's declared default width without resetting the clock.
    pub fn reset_size(&mut self) -> Result<SizeChange, String> {
        self.change_size(self.asset.manifest.default_columns)
    }

    /// Select the largest generated grid that fits both logical limits.
    /// Returns an error, preserving the old selection, when no size fits.
    pub fn fit_dimensions(&mut self, max_columns: u32, max_rows: u32) -> Result<SizeChange, String> {
        if max_columns == 0 || max_rows == 0 {return Err("Grid limits must be positive".into());}
        let selected = self.asset.manifest.fitting_columns(self.animation.current_frame(), max_columns, max_rows).ok_or("No generated size fits the requested dimensions")?;
        self.change_size(selected)
    }

    /// Select a grid that fits a pixel container at the supplied render metrics.
    /// Width and height use logical/CSS pixels, not device pixels. This chooses
    /// a rendition without modifying the font, render configuration or clock.
    pub fn fit_to_container(&mut self, width: f64, height: f64, config: &RenderConfig) -> Result<SizeChange, String> {
        let sizing = &config.sizing;
        let cell_width = sizing.char_width(config.font_size);
        let cell_height = sizing.line_height(config.font_size);
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 || !config.font_size.is_finite() || config.font_size <= 0.0 || !cell_width.is_finite() || !cell_height.is_finite() || cell_width <= 0.0 || cell_height <= 0.0 || !sizing.padding.is_finite() || sizing.padding < 0.0 {
            return Err("Container and render metrics must be finite and positive, with nonnegative padding".into());
        }
        let available = self.available_sizes()?;
        let columns = available.iter().rev().find(|size| {
            f64::from(size.columns) * cell_width + sizing.padding <= width
                && f64::from(size.rows) * cell_height + sizing.padding <= height
        }).ok_or("No generated size fits the pixel container")?.columns;
        self.change_size(columns)
    }

    pub fn set_cache_limits(&mut self, frames: usize, bytes: usize) -> Result<(), String> {
        if frames == 0 || bytes == 0 {return Err("Cache limits must be positive".into());}
        self.capacity = frames;
        self.byte_capacity = bytes;
        self.evict();
        Ok(())
    }

    pub fn select_columns(&mut self, columns: u32) -> Result<(), String> {
        self.load(columns, self.animation.current_frame())?;
        self.columns = columns;
        self.follow_map = false;
        Ok(())
    }

    pub fn fit_columns(&mut self, available: u32) -> Result<u32, String> {
        if available == 0 {return Err("Available columns must be positive".into());}
        let columns = self.asset.manifest.variants.iter().rev().find(|variant| variant.columns <= available).unwrap_or(&self.asset.manifest.variants[0]).columns;
        self.select_columns(columns)?;
        Ok(columns)
    }

    pub fn current(&mut self) -> Result<&Frame, String> {
        let index = self.animation.current_frame();
        self.load(self.selected_columns(), index)?;
        Ok(&self.cache.back().expect("load inserts the current frame").2)
    }

    /// Predecode a bounded window across sizes for immediate manual switching.
    pub fn prepare(&mut self, lookahead: usize) -> Result<(), String> {
        let index = self.animation.current_frame();
        let end = index.saturating_add(lookahead.min(120)).saturating_add(1).min(self.asset.manifest.frame_count as usize);
        let columns = self.asset.manifest.variants.iter().map(|variant| variant.columns).collect::<Vec<_>>();
        for frame in (index..end).rev() {
            for &columns in &columns {self.load(columns, frame)?;}
        }
        Ok(())
    }

    fn dimensions_for(&self, columns: u32) -> Result<FrameSize, String> {
        let variant = self.asset.manifest.variants.iter().find(|variant| variant.columns == columns).ok_or("Requested size is missing")?;
        let rows = *variant.rows.get(self.animation.current_frame()).ok_or("Frame index is out of range")?;
        Ok(FrameSize {columns, rows})
    }

    fn change_size(&mut self, columns: u32) -> Result<SizeChange, String> {
        let previous = self.current_size()?;
        let current = self.dimensions_for(columns)?;
        if current != previous {
            // Decode and check cache budgets before publishing the selection.
            self.select_columns(columns)?;
        }
        Ok(SizeChange {previous, current})
    }

    fn load(&mut self, columns: u32, index: usize) -> Result<(), String> {
        if let Some(position) = self.cache.iter().position(|item| item.0 == columns && item.1 == index) {
            let cached = self.cache.remove(position).unwrap();
            self.cache.push_back(cached);
            return Ok(());
        }
        let frame = self.asset.frame(columns, index)?;
        let bytes = frame_bytes(&frame);
        if bytes > self.byte_capacity {return Err("Frame exceeds cache byte budget".into());}
        self.cached_bytes += bytes;
        self.cache.push_back((columns, index, frame));
        self.evict();
        Ok(())
    }

    fn evict(&mut self) {
        while self.cache.len() > self.capacity || self.cached_bytes > self.byte_capacity {
            if let Some((_, _, frame)) = self.cache.pop_front() {self.cached_bytes -= frame_bytes(&frame);}
        }
    }
}
