#![cfg(feature = "multisize")]
use cascii_core_view::{LoopMode, MultiSizeAsset, MultiSizePlayer};

const BYTES: &[u8] = include_bytes!("fixtures/multisize.bin");

#[test]
fn rust_encoder_fixture_preserves_sizes_glyphs_colors_and_backgrounds() {
    let asset = MultiSizeAsset::parse(BYTES).unwrap();
    let json: serde_json::Value = serde_json::from_slice(include_bytes!("fixtures/multisize.json")).unwrap();
    for variant in &asset.manifest.variants {
        for index in 0..4 {
            let frame = asset.frame(variant.columns, index).unwrap();
            let stored = json["variants"].as_array().unwrap().iter().find(|item| item["columns"] == variant.columns).unwrap();
            assert_eq!(frame.content, stored["frames"][index]["text"].as_str().unwrap());
            let encoded = cascii_core_view::encode_cframe(frame.cframe.as_ref().unwrap()).unwrap();
            assert_eq!(encoded, stored["frames"][index]["cframe"].as_array().unwrap().iter().map(|byte| byte.as_u64().unwrap() as u8).collect::<Vec<_>>());
            assert_eq!(frame.cframe.as_ref().unwrap().has_background(), index % 2 == 0);
        }
    }
}

#[test]
fn playback_keeps_position_speed_range_loop_and_pause_state_when_switching() {
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(BYTES).unwrap());
    player.animation.set_fps(12);
    player.animation.set_range(0.25, 1.0);
    player.animation.set_loop_mode(LoopMode::Once);
    player.animation.seek(0.5);
    player.animation.play();
    let index = player.animation.current_frame();
    let interval = player.animation.interval_ms();
    player.prepare(3).unwrap();
    for columns in [2, 3, 2, 3] {
        player.select_columns(columns).unwrap();
        assert_eq!(player.animation.current_frame(), index);
        assert_eq!(player.animation.interval_ms(), interval);
        assert_eq!(player.animation.range(), (0.25, 1.0));
        assert!(player.animation.is_playing());
        assert_eq!(player.current().unwrap().dimensions().0, columns as usize);
    }
    player.animation.tick();
    assert_eq!(player.animation.current_frame(), index + 1);
    player.animation.pause();
    player.select_columns(2).unwrap();
    assert!(!player.animation.is_playing());
}

#[test]
fn cache_is_bounded_and_failed_selections_keep_the_current_size() {
    let mut player = MultiSizePlayer::new(MultiSizeAsset::parse(BYTES).unwrap());
    player.set_cache_limits(2, 1000).unwrap();
    for index in 0..30 {
        player.animation.seek((index % 4) as f64 / 3.0);
        player.select_columns(if index % 2 == 0 {2} else {3}).unwrap();
        assert_eq!(player.current().unwrap().dimensions().0, player.selected_columns() as usize);
        assert!(player.cached_frame_count() <= 2);
        assert!(player.cached_bytes() <= 1000);
    }
    let previous = player.selected_columns();
    assert!(player.select_columns(55).is_err());
    assert_eq!(player.selected_columns(), previous);
    player.set_cache_limits(2, 1).unwrap();
    assert!(player.select_columns(2).is_err());
    assert_eq!(player.selected_columns(), previous);
    assert_eq!(player.fit_columns(0).unwrap_err(), "Available columns must be positive");
}

#[test]
fn corrupt_and_truncated_containers_fail_without_panicking() {
    for end in [0, 4, 15, 16, BYTES.len() - 1] {
        assert!(MultiSizeAsset::parse(&BYTES[..end]).is_err());
    }
    let metadata_len = u32::from_le_bytes(BYTES[8..12].try_into().unwrap()) as usize;
    let start = 16 + metadata_len;
    for offset in [4, 8, 12, start, start + 4, start + 8, start + 16, start + 20] {
        let mut corrupt = BYTES.to_vec();
        corrupt[offset] ^= 255;
        assert!(MultiSizeAsset::parse(&corrupt).is_err());
    }
    let mut trailing = BYTES.to_vec();
    trailing.push(0);
    assert!(MultiSizeAsset::parse(&trailing).is_err());
}

#[test]
fn saved_map_follows_seek_and_live_edits_preserve_clock() {
    let mut player=MultiSizePlayer::new(MultiSizeAsset::parse(include_bytes!("fixtures/multisize-v2.bin")).unwrap());
    assert_eq!(player.current().unwrap().dimensions().0,2);
    player.animation.seek(1.0/3.0);
    assert_eq!(player.selected_columns(),3);assert_eq!(player.current().unwrap().dimensions().0,3);
    let frame=player.animation.current_frame();
    player.set_column_map(vec![3,2,3,2]).unwrap();assert_eq!(player.current().unwrap().dimensions().0,2);
    assert_eq!(player.animation.current_frame(),frame);
    player.select_columns(3).unwrap();assert_eq!(player.current().unwrap().dimensions().0,3);
    player.follow_column_map();assert_eq!(player.current().unwrap().dimensions().0,2);
    assert!(player.set_column_map(vec![9;4]).is_err());
}

#[test]
fn following_a_legacy_asset_resumes_the_default_width() {
    let mut player=MultiSizePlayer::new(MultiSizeAsset::parse(BYTES).unwrap());
    player.select_columns(2).unwrap();player.follow_column_map();assert_eq!(player.selected_columns(),3);
}

#[test]
fn indexed_source_reads_only_metadata_and_the_requested_frame() {
    use cascii_core_view::multisize::{FrameBytes, IndexedMultiSizeAsset};
    use std::{borrow::Cow, cell::RefCell, ops::Range};

    struct Reads<'a>(&'a RefCell<Vec<Range<usize>>>);
    impl FrameBytes for Reads<'_> {
        fn read_range(&self, range: Range<usize>) -> Result<Cow<'_, [u8]>, String> {
            self.0.borrow_mut().push(range.clone());
            Ok(Cow::Borrowed(&BYTES[range]))
        }
    }
    let reads = RefCell::new(Vec::new());
    let asset = IndexedMultiSizeAsset::open(Reads(&reads), BYTES.len()).unwrap();
    let payload_start = reads.borrow().last().unwrap().end;
    assert!(payload_start < BYTES.len());
    reads.borrow_mut().clear();
    let frame = asset.frame(3, 2).unwrap();
    assert_eq!(frame.dimensions().0, 3);
    assert_eq!(reads.borrow().len(), 1);
    assert!(reads.borrow()[0].start >= payload_start);
}
