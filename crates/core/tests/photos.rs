//! F7 photo and video review: sources, groups, certainty and the best shot, with
//! generated images only (T-7.1 to T-7.6, and S2 for hashing).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::FileSystem;
use unhoard_core::photos::{
    Certainty, Features, Gray, GroupKind, Source, best_shot, bursts, by_month, cloud_duplicates,
    dhash, exact_duplicates_local, hamming, laplacian_variance, media_index, near_duplicates,
    parse_name, video_duplicates,
};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::time::days_from_civil;

fn ctx(fs: &FakeFs) -> ScanContext {
    let env: BTreeMap<String, String> =
        [("USERPROFILE".to_string(), r"C:\Users\u".to_string())].into();
    ScanContext {
        rules: Rules::empty(),
        roots: Roots::new(env, None, vec![r"C:\Users\u\OneDrive".into()]),
        now: fs.now(),
    }
}

fn at(y: i64, m: u32, d: u32, hh: i64, mm: i64, ss: i64) -> i64 {
    days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss
}

#[test]
fn f7_file_name_patterns_map_to_the_right_source() {
    let cases = [
        (
            "IMG-20190704-WA0012.jpg",
            Source::WhatsApp,
            Some(at(2019, 7, 4, 0, 0, 0)),
        ),
        (
            "VID-20200101-WA0001.mp4",
            Source::WhatsApp,
            Some(at(2020, 1, 1, 0, 0, 0)),
        ),
        (
            "20180812_153012.jpg",
            Source::Samsung,
            Some(at(2018, 8, 12, 15, 30, 12)),
        ),
        (
            "PXL_20230115_101530123.jpg",
            Source::Pixel,
            Some(at(2023, 1, 15, 10, 15, 30)),
        ),
        ("IMG_4321.HEIC", Source::IPhone, None),
        ("IMG_E4321.JPG", Source::IPhone, None),
        (
            "WP_20140302_001.jpg",
            Source::WindowsPhone,
            Some(at(2014, 3, 2, 0, 0, 0)),
        ),
        ("Screenshot_20210505-101010.png", Source::Screenshot, None),
        (
            "Schermafbeelding 2024-02-01 101010.png",
            Source::Screenshot,
            None,
        ),
        ("Screenshot 2024-02-01 101010.png", Source::Screenshot, None),
        ("holiday.jpg", Source::Unknown, None),
        ("IMG-2019-WA.jpg", Source::Unknown, None),
    ];
    for (name, source, taken) in cases {
        let p = parse_name(name);
        assert_eq!(p.source, source, "{name}");
        if taken.is_some() {
            assert_eq!(p.taken, taken, "{name}");
        }
    }
    assert!(parse_name("VID-20200101-WA0001.mp4").video);
}

fn photo_tree() -> (FakeFs, unhoard_core::tree::Tree, ScanContext) {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\Pictures").unwrap();
    let mut a = vec![7u8; 200_000];
    let b = a.clone();
    let mut c = a.clone();
    c[100_000] = 8; // same size, same first and last 64 KB, different middle
    a[0] = 7;
    fs.add_file_with(
        r"C:\Users\u\Pictures\a.jpg",
        FileOpts {
            data: a,
            ..FileOpts::default()
        },
    );
    fs.add_file_with(
        r"C:\Users\u\Pictures\b.jpg",
        FileOpts {
            data: b,
            ..FileOpts::default()
        },
    );
    fs.add_file_with(
        r"C:\Users\u\Pictures\c.jpg",
        FileOpts {
            data: c,
            ..FileOpts::default()
        },
    );
    fs.create_dir_all(r"C:\Users\u\OneDrive\Pictures").unwrap();
    fs.add_placeholder(r"C:\Users\u\OneDrive\Pictures\x.jpg", 3_000_000, true);
    fs.add_placeholder(r"C:\Users\u\OneDrive\Pictures\y.jpg", 3_000_000, true);
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\Users\u", &c, &ScanControl::default()).unwrap();
    (fs, t, c)
}

#[test]
fn f7_s2_local_exact_duplicates_need_equal_full_hashes_and_cloud_files_are_never_read() {
    let (fs, t, c) = photo_tree();
    let media = media_index(&t, &c);
    assert_eq!(media.len(), 5);
    let groups = exact_duplicates_local(&fs, &media);
    assert_eq!(groups.len(), 1);
    let names: Vec<&str> = groups[0]
        .items
        .iter()
        .map(|&i| media[i].name.as_str())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"a.jpg") && names.contains(&"b.jpg"));
    assert_eq!(groups[0].certainty, Certainty::Identical);
    assert!(
        fs.data_opens().iter().all(|p| !p.contains("OneDrive")),
        "s2: a cloud file was read"
    );
}

#[test]
fn f7_cloud_matches_are_almost_certainly_identical_never_identical() {
    let (_, t, c) = photo_tree();
    let media = media_index(&t, &c);
    let mut features = vec![Features::default(); media.len()];
    for (i, m) in media.iter().enumerate() {
        if m.cloud_only {
            features[i].thumb_hash = Some(0xABCD);
        }
    }
    let groups = cloud_duplicates(&media, &features);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].certainty, Certainty::AlmostCertainlyIdentical);
    assert_eq!(groups[0].kind, GroupKind::ExactDuplicates);
}

#[test]
fn f7_bursts_group_photos_from_the_same_source_within_the_time_window() {
    let names = [
        "20180812_153012.jpg",
        "20180812_153013.jpg",
        "20180812_153014.jpg",
        "20180812_160000.jpg",
        "PXL_20180812_153013000.jpg",
    ];
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\p").unwrap();
    for n in names {
        fs.add_file(&format!(r"C:\p\{n}"), 1000);
    }
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\p", &c, &ScanControl::default()).unwrap();
    let media = media_index(&t, &c);
    let groups = bursts(&media);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].items.len(), 3);
    assert!(
        groups[0]
            .items
            .iter()
            .all(|&i| media[i].parsed.source == Source::Samsung)
    );
}

#[test]
fn f7_chat_media_and_screenshots_are_grouped_by_month() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\w").unwrap();
    for n in [
        "IMG-20190704-WA0012.jpg",
        "IMG-20190710-WA0001.jpg",
        "IMG-20190801-WA0001.jpg",
        "Screenshot_20210505-101010.png",
    ] {
        fs.add_file(&format!(r"C:\w\{n}"), 100);
    }
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\w", &c, &ScanControl::default()).unwrap();
    let media = media_index(&t, &c);
    let groups = by_month(&media);
    let chat: Vec<_> = groups
        .iter()
        .filter(|g| g.kind == GroupKind::ChatMedia)
        .collect();
    assert_eq!(chat.len(), 2);
    assert!(
        chat.iter()
            .any(|g| g.label == "2019-07" && g.items.len() == 2)
    );
    assert_eq!(
        groups
            .iter()
            .filter(|g| g.kind == GroupKind::Screenshots)
            .count(),
        1
    );
}

/// A generated photo: smooth gradients with a few shapes, from a seed.
fn picture(seed: u32, w: u32, h: u32) -> Gray {
    let mut data = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = match seed % 3 {
                0 => (x * 255 / w) as i32,
                1 => (y * 255 / h) as i32,
                _ => (((x / 8 + y / 8) % 2) * 200) as i32,
            };
            let shape = if (x as i32 - (w as i32) / 3).pow(2) + (y as i32 - (h as i32) / 2).pow(2)
                < (w as i32 / 6).pow(2)
            {
                60
            } else {
                0
            };
            data[(y * w + x) as usize] = (v + shape).clamp(0, 255) as u8;
        }
    }
    Gray {
        width: w,
        height: h,
        data,
    }
}

fn blur(g: &Gray, passes: u32) -> Gray {
    let mut cur = g.clone();
    for _ in 0..passes {
        let mut next = cur.clone();
        for y in 1..g.height - 1 {
            for x in 1..g.width - 1 {
                let mut s = 0u32;
                for dy in 0..3 {
                    for dx in 0..3 {
                        s += cur.data[((y + dy - 1) * g.width + x + dx - 1) as usize] as u32;
                    }
                }
                next.data[(y * g.width + x) as usize] = (s / 9) as u8;
            }
        }
        cur = next;
    }
    cur
}

#[test]
fn f7_perceptual_hash_groups_near_duplicates_and_separates_different_images() {
    let base = picture(0, 96, 72);
    let mut brighter = base.clone();
    for p in brighter.data.iter_mut() {
        *p = p.saturating_add(6);
    }
    let other = picture(1, 96, 72);
    assert!(
        hamming(dhash(&base), dhash(&brighter)) <= 6,
        "a re-saved copy must stay close"
    );
    assert!(
        hamming(dhash(&base), dhash(&other)) > 12,
        "a different picture must be far"
    );

    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\n").unwrap();
    for (i, n) in ["one.jpg", "two.jpg", "three.jpg"].iter().enumerate() {
        fs.add_file(&format!(r"C:\n\{n}"), 1000 + i as u64);
    }
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\n", &c, &ScanControl::default()).unwrap();
    let media = media_index(&t, &c);
    let hash_of = |name: &str| match name {
        "one.jpg" => dhash(&base),
        "two.jpg" => dhash(&brighter),
        _ => dhash(&other),
    };
    let features: Vec<Features> = media
        .iter()
        .map(|m| Features {
            thumb_hash: Some(hash_of(&m.name)),
            ..Features::default()
        })
        .collect();
    let groups = near_duplicates(&media, &features, &[]);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].items.len(), 2);
    assert_eq!(groups[0].certainty, Certainty::VerySimilar);
}

#[test]
fn f7_the_best_shot_prefers_the_sharpest_image() {
    let sharp = picture(2, 96, 72);
    let soft = blur(&sharp, 3);
    assert!(laplacian_variance(&sharp) > laplacian_variance(&soft) * 2.0);
    let features = vec![
        Features {
            sharpness: Some(laplacian_variance(&soft)),
            size: Some((4000, 3000)),
            ..Features::default()
        },
        Features {
            sharpness: Some(laplacian_variance(&sharp)),
            size: Some((4000, 3000)),
            ..Features::default()
        },
        Features {
            sharpness: Some(laplacian_variance(&soft) * 1.01),
            size: Some((4000, 3000)),
            ..Features::default()
        },
    ];
    let (keeper, reasons) = best_shot(&[0, 1, 2], &features, &[1000, 1000, 1000]).unwrap();
    assert_eq!(keeper, 1);
    assert!(!reasons.is_empty());
}

#[test]
fn f7_video_duplicates_match_on_size_duration_and_resolution() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\v").unwrap();
    for n in ["a.mp4", "b.mp4", "c.mp4"] {
        fs.add_file(&format!(r"C:\v\{n}"), 50_000_000);
    }
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\v", &c, &ScanControl::default()).unwrap();
    let media = media_index(&t, &c);
    let features: Vec<Features> = media
        .iter()
        .map(|m| Features {
            duration_ms: Some(if m.name == "c.mp4" { 9_000 } else { 61_000 }),
            size: Some((1920, 1080)),
            ..Features::default()
        })
        .collect();
    let groups = video_duplicates(&media, &features);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].items.len(), 2);
}
