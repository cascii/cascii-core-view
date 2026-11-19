#![cfg(feature = "multisize")]
use cascii_core_view::{encode_cframe, CFrameData, FrameSize, LoopMode, MultiSizeAsset, MultiSizeManifest, MultiSizePlayer, RenderConfig, SizeMatch, SizeVariant};

const COLUMNS: &[u32] = &[55, 58, 61, 67, 73, 80];

/// A real encoded asset with irregular size steps and changing frame heights.
fn asset_bytes(columns: &[u32]) -> Vec<u8> {
    let manifest = MultiSizeManifest {
        format: "cascii-multisize".into(), schema_version: 1, frame_count: 4, fps: 30,
        column_map: None, default_columns: 73, font_ratio: 0.7, luminance: 20, ascii_chars: "#".into(),
        text: false, color: true, audio: None, outputs: vec!["frames.bin".into()],
        variants: columns.iter().map(|&columns| SizeVariant {columns, rows: (0..4).map(|index| columns / 10 + 1 + index % 2).collect()}).collect(),
    };
    manifest.validate().unwrap();
    let json = serde_json::to_vec(&manifest).unwrap();
    let mut index = Vec::new();
    let mut payload = Vec::new();
    for frame in 0..manifest.frame_count {
        for variant in &manifest.variants {
            let rows = variant.rows[frame as usize];
            let cells = variant.columns as usize * rows as usize;
            let cframe = CFrameData::new(variant.columns, rows, vec![b'#'; cells], vec![128; cells * 3]);
            let encoded = encode_cframe(&cframe).unwrap();
            index.extend_from_slice(&variant.columns.to_le_bytes());
            index.extend_from_slice(&frame.to_le_bytes());
            index.extend_from_slice(&(payload.len() as u64).to_le_bytes());
            index.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            index.extend_from_slice(&1u32.to_le_bytes());
            payload.extend_from_slice(&encoded);
        }
    }
    let mut bytes = b"CSMV".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(manifest.frame_count * columns.len() as u32).to_le_bytes());
    bytes.extend(json);
    bytes.extend(index);
    bytes.extend(payload);
    bytes
}

#[test]
fn smaller_and_larger_step_through_generated_sizes_and_stop_at_boundaries() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    for expected in [67, 61, 58, 55] {
        let change = player.smaller().unwrap();
        assert!(change.changed());
        assert_eq!(change.current.columns, expected);
        assert_eq!(player.current().unwrap().dimensions().0, expected as usize);
    }
    assert!(!player.can_smaller());
    let unchanged = player.smaller().unwrap();
    assert!(!unchanged.changed());
    assert_eq!(unchanged.previous, unchanged.current);
    for expected in [58, 61, 67, 73, 80] {
        assert_eq!(player.larger().unwrap().current.columns, expected);
    }
    assert!(!player.can_larger());
    assert!(!player.larger().unwrap().changed());
    let reset = player.reset_size().unwrap();
    assert_eq!(reset.previous.columns, 80);
    assert_eq!(reset.current, FrameSize {columns: 73, rows: 8});
    assert!(!player.reset_size().unwrap().changed());
}

#[test]
fn direct_and_matched_sizes_have_explicit_rounding_and_tie_behavior() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    assert_eq!(player.set_size(80).unwrap().current.columns, 80);
    assert_eq!(player.resize_to(64, SizeMatch::Nearest).unwrap().current.columns, 61);
    assert_eq!(player.resize_to(70, SizeMatch::Nearest).unwrap().current.columns, 67);
    assert_eq!(player.resize_to(72, SizeMatch::Nearest).unwrap().current.columns, 73);
    assert_eq!(player.resize_to(64, SizeMatch::Floor).unwrap().current.columns, 61);
    assert_eq!(player.resize_to(64, SizeMatch::Ceil).unwrap().current.columns, 67);
    assert_eq!(player.resize_to(u32::MAX, SizeMatch::Nearest).unwrap().current.columns, 80);
    assert_eq!(player.resize_to(1, SizeMatch::Nearest).unwrap().current.columns, 55);
    assert_eq!(player.resize_to(u32::MAX, SizeMatch::Floor).unwrap().current.columns, 80);
    assert_eq!(player.resize_to(1, SizeMatch::Ceil).unwrap().current.columns, 55);
    for matching in [SizeMatch::Exact, SizeMatch::Nearest, SizeMatch::Floor, SizeMatch::Ceil] {
        assert!(player.resize_to(0, matching).is_err());
        assert_eq!(player.selected_columns(), 55);
    }
    assert!(player.set_size(64).is_err());
    assert!(player.resize_to(54, SizeMatch::Floor).is_err());
    assert!(player.resize_to(81, SizeMatch::Ceil).is_err());
    assert_eq!(player.selected_columns(), 55);
}

#[test]
fn exact_dimensions_accept_picker_values_and_reject_unavailable_grids() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    let selected = player.available_sizes().unwrap()[0];
    assert_eq!(player.set_dimensions(selected).unwrap().current, selected);
    assert!(player.set_dimensions(FrameSize {columns: 73, rows: 99}).is_err());
    assert!(player.set_dimensions(FrameSize {columns: 64, rows: 8}).is_err());
    assert_eq!(player.selected_columns(), 55);
    player.animation.seek(1.0 / 3.0);
    assert!(player.set_dimensions(selected).is_err());
    let current = player.available_sizes().unwrap()[0];
    assert_eq!(current, FrameSize {columns: 55, rows: 7});
    assert!(!player.set_dimensions(current).unwrap().changed());
}

#[test]
fn size_state_is_a_read_only_snapshot_for_ui_controls() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    let state = player.size_state().unwrap();
    assert_eq!(state.current, FrameSize {columns: 73, rows: 8});
    assert_eq!(state.available.iter().map(|size| size.columns).collect::<Vec<_>>(), COLUMNS);
    assert_eq!(state.default_columns, 73);
    assert!(state.can_smaller && state.can_larger);
    assert_eq!(player.cached_frame_count(), 0);
    player.animation.tick();
    player.animation.seek(1.0 / 3.0);
    assert_eq!(player.current_size().unwrap(), FrameSize {columns: 73, rows: 9});
    assert_eq!(player.available_sizes().unwrap()[0], FrameSize {columns: 55, rows: 7});
    assert_eq!(player.cached_frame_count(), 0);
    let serialized = serde_json::to_string(&player.size_state().unwrap()).unwrap();
    let decoded: cascii_core_view::SizeState = serde_json::from_str(&serialized).unwrap();
    assert_eq!(decoded, player.size_state().unwrap());
}

#[test]
fn fitting_uses_both_limits_and_the_current_frames_actual_height() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    assert_eq!(player.fit_dimensions(80, 8).unwrap().current.columns, 73);
    assert_eq!(player.fit_dimensions(65, 8).unwrap().current.columns, 61);
    assert_eq!(player.fit_dimensions(80, 7).unwrap().current.columns, 67);
    assert!(player.fit_dimensions(80, 5).is_err());
    assert_eq!(player.selected_columns(), 67);
    player.animation.seek(1.0 / 3.0);
    assert_eq!(player.fit_dimensions(80, 8).unwrap().current, FrameSize {columns: 67, rows: 8});
    assert!(player.fit_dimensions(0, 8).is_err());
    assert!(player.fit_dimensions(80, 0).is_err());
}

#[test]
fn pixel_fitting_respects_font_metrics_padding_and_exact_boundaries() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    let mut config = RenderConfig::new(10.0);
    config.sizing.char_width_ratio = 0.5;
    config.sizing.line_height_ratio = 1.0;
    config.sizing.padding = 20.0;
    assert_eq!(player.fit_to_container(385.0, 100.0, &config).unwrap().current.columns, 73);
    assert_eq!(player.fit_to_container(420.0, 100.0, &config).unwrap().current.columns, 73);
    assert_eq!(player.fit_to_container(420.0, 110.0, &config).unwrap().current.columns, 80);
    assert_eq!(config.font_size, 10.0);
    player.animation.seek(1.0 / 3.0);
    assert_eq!(player.fit_to_container(420.0, 100.0, &config).unwrap().current.columns, 67);
    assert!(player.fit_to_container(20.0, 20.0, &config).is_err());
    assert_eq!(player.selected_columns(), 67);
    config.font_size = 13.0;
    config.sizing.char_width_ratio = 0.6;
    let width = 73.0 * config.sizing.char_width(config.font_size) + config.sizing.padding;
    assert_eq!(player.fit_to_container(width, 200.0, &config).unwrap().current.columns, 73);
}

#[test]
fn invalid_pixel_metrics_are_rejected_without_changing_the_selection() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    let config = RenderConfig::default();
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(player.fit_to_container(invalid, 600.0, &config).is_err());
        assert!(player.fit_to_container(800.0, invalid, &config).is_err());
        let mut bad_font = config.clone();
        bad_font.font_size = invalid;
        assert!(player.fit_to_container(800.0, 600.0, &bad_font).is_err());
        let mut bad_width = config.clone();
        bad_width.sizing.char_width_ratio = invalid;
        assert!(player.fit_to_container(800.0, 600.0, &bad_width).is_err());
        let mut bad_height = config.clone();
        bad_height.sizing.line_height_ratio = invalid;
        assert!(player.fit_to_container(800.0, 600.0, &bad_height).is_err());
        assert_eq!(player.selected_columns(), 73);
    }
    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        let mut bad_padding = config.clone();
        bad_padding.sizing.padding = invalid;
        assert!(player.fit_to_container(800.0, 600.0, &bad_padding).is_err());
    }
}

#[test]
fn every_control_keeps_playing_or_paused_state_speed_range_and_position() {
    let bytes = asset_bytes(COLUMNS);
    for playing in [false, true] {
        let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
        player.animation.set_fps(12);
        player.animation.set_range(0.25, 1.0);
        player.animation.set_loop_mode(LoopMode::Once);
        player.animation.seek(0.5);
        if playing {player.animation.play();}
        let snapshot = (player.animation.current_frame(), player.animation.fps(), player.animation.range(), player.animation.loop_mode(), player.animation.state());
        player.smaller().unwrap();
        player.larger().unwrap();
        player.set_size(80).unwrap();
        player.set_dimensions(FrameSize {columns: 73, rows: 8}).unwrap();
        player.resize_to(64, SizeMatch::Nearest).unwrap();
        player.fit_dimensions(80, 8).unwrap();
        player.fit_to_container(800.0, 600.0, &RenderConfig::default()).unwrap();
        player.reset_size().unwrap();
        assert_eq!((player.animation.current_frame(), player.animation.fps(), player.animation.range(), player.animation.loop_mode(), player.animation.state()), snapshot);
        assert_eq!(player.animation.tick(), playing);
        assert_eq!(player.animation.current_frame(), snapshot.0 + usize::from(playing));
    }
}

#[test]
fn failed_decoding_and_cache_budgets_keep_the_previous_size_and_frame() {
    let mut bytes = asset_bytes(COLUMNS);
    let metadata_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let entry = 16 + metadata_len + 5 * 24;
    let offset = u64::from_le_bytes(bytes[entry + 8..entry + 16].try_into().unwrap()) as usize;
    bytes[16 + metadata_len + count * 24 + offset + 8] = 0;
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    player.set_size(55).unwrap();
    let original = player.current().unwrap().content.clone();
    assert!(player.set_size(80).is_err());
    assert_eq!(player.selected_columns(), 55);
    assert_eq!(player.current().unwrap().content, original);
    player.set_cache_limits(4, 2000).unwrap();
    let cached = player.cached_bytes();
    assert!(player.set_size(73).is_err());
    assert_eq!(player.selected_columns(), 55);
    assert_eq!(player.cached_bytes(), cached);
    assert_eq!(player.current().unwrap().content, original);
}

#[test]
fn a_single_variant_has_no_step_controls_and_no_op_requests_do_not_decode() {
    let bytes = asset_bytes(&[73]);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    let state = player.size_state().unwrap();
    assert!(!state.can_smaller && !state.can_larger);
    player.set_cache_limits(1, 1).unwrap();
    assert!(!player.smaller().unwrap().changed());
    assert!(!player.larger().unwrap().changed());
    assert!(!player.reset_size().unwrap().changed());
    assert!(!player.set_size(73).unwrap().changed());
    assert_eq!(player.cached_frame_count(), 0);
}

#[test]
fn invalid_external_frame_positions_fail_without_panicking() {
    let bytes = asset_bytes(COLUMNS);
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(&bytes).unwrap());
    player.animation.set_frame_count(10);
    player.animation.seek(1.0);
    assert!(player.current_size().is_err());
    assert!(player.available_sizes().is_err());
    assert!(player.size_state().is_err());
    assert!(player.smaller().is_err());
    assert!(player.larger().is_err());
    assert!(player.set_size(80).is_err());
    assert!(player.fit_dimensions(80, 10).is_err());
}
