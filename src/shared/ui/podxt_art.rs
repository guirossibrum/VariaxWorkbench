//! Exact PODxt Live stage artwork for the catalogue models and fixed blocks.
//! The model-to-image relationships are documented in assets/ui/SOURCES.md.

use eframe::egui;
use image::ImageFormat;

use super::assets::{self, UiImage};

/// The stage block or the cabinet preview in the Amp controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Block {
    Amp,
    Cab,
    Gate,
    Wah,
    Stomp,
    Compressor,
    Eq,
    Volume,
    Mod,
    Delay,
    Reverb,
}

pub const FRAME_KEY: &str = "pod-farm/stage-frame.png";

/// Upload an embedded stage PNG. Unknown or malformed keys return a missing image.
pub fn load(ctx: &egui::Context, key: &str) -> UiImage {
    match png(key)
        .and_then(|bytes| image::load_from_memory_with_format(bytes, ImageFormat::Png).ok())
    {
        Some(decoded) => assets::from_decoded(ctx, key, decoded),
        None => UiImage::missing(key),
    }
}

macro_rules! stage_art {
    ($($block:ident { $($file:literal => $models:pat,)+ })+) => {
        /// Look up the artwork using the block and its exact stored model ID.
        ///
        /// An absent or unrecognized model, including No Cab, has no key.
        /// Use the same key for a configured block when its enable flag is off.
        /// Apply the bypass tint in the caller.
        pub fn model_key(block: Block, model_id: Option<u32>) -> Option<&'static str> {
            match (block, model_id) {
                $($((Block::$block, Some($models)) => Some(concat!("pod-farm/", $file)),)+)+
                _ => None,
            }
        }

        fn png(key: &str) -> Option<&'static [u8]> {
            match key {
                FRAME_KEY => Some(include_bytes!("../../../assets/ui/pod-farm/stage-frame.png")),
                $($(concat!("pod-farm/", $file) => Some(include_bytes!(
                    concat!("../../../assets/ui/pod-farm/", $file)
                )),)+)+
                _ => None,
            }
        }
    };
}

stage_art! {
    Amp {
        "amp-no-amp.png" => 0x00020000,
        "amp-line-6-clean.png" => 0x00020001,
        "amp-cc-l6-sparkle.png" => 0x00020002,
        "amp-cc-l6-twang.png" => 0x00020003,
        "amp-cc-l6-bayou.png" => 0x00020004,
        "amp-line-6-jts-45.png" => 0x00020005,
        "amp-line-6-class-a.png" => 0x00020006,
        "amp-line-6-mood.png" => 0x00020007,
        "amp-spinal-puppet.png" => 0x00020008,
        "amp-ms-l6-throttle.png" => 0x00020009,
        "amp-line-6-chem-x.png" => 0x0002000a,
        "amp-cc-l6-purge.png" => 0x0002000b,
        "amp-line-6-insane.png" => 0x0002000c,
        "amp-ms-l6-octone.png" => 0x0002000d,
        "amp-line-6-aco-2.png" => 0x0002000f,
        "amp-zen-master.png" => 0x00020010,
        "amp-small-tweed.png" => 0x00020011,
        "amp-tweed-b-man.png" => 0x00020012,
        "amp-tiny-tweed.png" => 0x00020013,
        "amp-blackface-lux.png" => 0x00020014,
        "amp-double-verb.png" => 0x00020015,
        "amp-cc-mini-double.png" => 0x00020016,
        "amp-cc-gibtone-expo.png" => 0x00020017,
        "amp-two-tone.png" => 0x00020018,
        "amp-hiway-100.png" => 0x00020019,
        "amp-plexi-45.png" => 0x0002001a,
        "amp-plexi-lead-100.png" => 0x0002001b,
        "amp-cc-brit-bass.png" => 0x0002001c,
        "amp-plexi-jump-lead.png" => 0x0002001d,
        "amp-plexi-variac.png" => 0x0002001e,
        "amp-cc-brit-major.png" => 0x0002001f,
        "amp-brit-j-800.png" => 0x00020020,
        "amp-brit-jm-pre.png" => 0x00020021,
        "amp-match-chief.png" => 0x00020022,
        "amp-match-d-30.png" => 0x00020023,
        "amp-treadplate-dual.png" => 0x00020024,
        "amp-ms-diamondplate.png" => 0x00020025,
        "amp-cali-crunch.png" => 0x00020026,
        "amp-jazz-clean.png" => 0x00020027,
        "amp-cc-silver-twelve.png" => 0x00020028,
        "amp-solo-100.png" => 0x00020029,
        "amp-super-o.png" => 0x0002002a,
        "amp-cc-super-o-thunder.png" => 0x0002002b,
        "amp-class-a-15.png" => 0x0002002c,
        "amp-class-a-30-tb.png" => 0x0002002d,
        "amp-tube-preamp.png" => 0x0002002e,
        "amp-ms-bomber-uber.png" => 0x0002002f,
        "amp-cc-bomber-x-tc.png" => 0x00020030,
        "amp-ms-angel-p-ball.png" => 0x00020031,
        "amp-variax-acoustic.png" => 0x00020032,
        "amp-cc-l6-super-cln.png" => 0x00020033,
        "amp-cc-l6-superspark.png" => 0x00020034,
        "amp-ms-l6-sparkle-cln.png" => 0x00020035,
        "amp-cc-l6-crunch.png" => 0x00020036,
        "amp-ms-l6-smash.png" => 0x00020037,
        "amp-ms-l6-fuzz.png" => 0x00020038,
        "amp-ms-l6-chunk-chunk.png" => 0x00020039,
        "amp-ms-l6-big-bottom.png" => 0x0002003a,
        "amp-line-6-treadplate.png" => 0x0002003b,
        "amp-line-6-lunatic.png" => 0x0002003c,
        "amp-line-6-agro.png" => 0x0002003d,
        "amp-ms-connor-50.png" => 0x0002003e,
        "amp-cc-deity-crunch.png" => 0x0002003f,
        "amp-ms-deity-lead.png" => 0x00020040,
        "amp-ms-deity-s-son.png" => 0x00020041,
        "amp-cc-blackface-vibro.png" => 0x00020042,
        "amp-cc-double-show.png" => 0x00020043,
        "amp-cc-silverface-bass.png" => 0x00020044,
        "amp-ms-brit-silver.png" => 0x00020045,
        "amp-ms-brit-j-900-cln.png" => 0x00020046,
        "amp-ms-brit-j-900-dst.png" => 0x00020047,
        "amp-ms-brit-j-2000.png" => 0x00020048,
        "amp-ms-criminal.png" => 0x00020049,
        "amp-citrus-d-30.png" => 0x0002004a,
        "amp-l6-modern-hi-gain.png" => 0x0002004b,
        "amp-l6-boutique-1.png" => 0x0002004c,
        "amp-class-a-30-fawn.png" => 0x0002004d,
        "amp-brit-gain-18.png" => 0x0002004e,
        "amp-brit-j-2000-2.png" => 0x0002004f,
        "amp-bx-tube-preamp.png" => 0x00030001,
        "amp-bx-l6-classic-jazz.png" => 0x00030002,
        "amp-bx-l6-brit-invader.png" => 0x00030003,
        "amp-bx-l6-super-thor.png" => 0x00030004,
        "amp-bx-l6-frankenstein.png" => 0x00030005,
        "amp-bx-l6-ebony-lux.png" => 0x00030006,
        "amp-bx-l6-doppleganger.png" => 0x00030007,
        "amp-bx-l6-sub-dub.png" => 0x00030008,
        "amp-bx-amp-360.png" => 0x00030009,
        "amp-bx-jaguar.png" => 0x0003000a,
        "amp-bx-alchemist.png" => 0x0003000b,
        "amp-bx-rock-classic.png" => 0x0003000c,
        "amp-bx-flip-top.png" => 0x0003000d,
        "amp-bx-adam-and-eve.png" => 0x0003000e,
        "amp-bx-tweed-b-man.png" => 0x0003000f,
        "amp-bx-silverface-bass.png" => 0x00030010,
        "amp-bx-double-show.png" => 0x00030011,
        "amp-bx-eighties.png" => 0x00030012,
        "amp-bx-hiway-100.png" => 0x00030013,
        "amp-bx-hiway-200.png" => 0x00030014,
        "amp-bx-british-major.png" => 0x00030015,
        "amp-bx-british-bass.png" => 0x00030016,
        "amp-bx-california.png" => 0x00030017,
        "amp-bx-jazz-tone.png" => 0x00030018,
        "amp-bx-stadium.png" => 0x00030019,
        "amp-bx-studio-tone.png" => 0x0003001a,
        "amp-bx-motor-city.png" => 0x0003001b,
        "amp-bx-brit-class-a100.png" => 0x0003001c,
    }
    Cab {
        "cab-1x6-super-o.png" => 0x01020001,
        "cab-1x8-tweed.png" => 0x01020002,
        "cab-1x10-gibtone.png" => 0x01020003,
        "cab-1x10-g-brand.png" => 0x01020004,
        "cab-1x12-line-6.png" => 0x01020005,
        "cab-1x12-tweed.png" => 0x01020006,
        "cab-1x12-blackface.png" => 0x01020007,
        "cab-1x12-class-a.png" => 0x01020008,
        "cab-2x2-mini-t.png" => 0x01020009,
        "cab-2x12-line-6.png" => 0x0102000a,
        "cab-2x12-blackface.png" => 0x0102000b,
        "cab-2x12-match.png" => 0x0102000c,
        "cab-2x12-jazz.png" => 0x0102000d,
        "cab-2x12-class-a.png" => 0x0102000e,
        "cab-4x10-line-6.png" => 0x0102000f | 0x0103000d,
        "cab-4x10-tweed.png" => 0x01020010,
        "cab-4x12-line-6.png" => 0x01020011,
        "cab-4x12-green-20-s.png" => 0x01020012,
        "cab-4x12-green-25-s.png" => 0x01020013,
        "cab-4x12-brit-t75.png" => 0x01020014,
        "cab-4x12-brit-v30-s.png" => 0x01020015,
        "cab-4x12-treadplate.png" => 0x01020016,
        "cab-1x15-thunder.png" => 0x01020018,
        "cab-2x12-wishbook.png" => 0x0102001a,
        "cab-bx-1x12-boutique.png" => 0x01030001,
        "cab-bx-1x12-motor-city.png" => 0x01030002,
        "cab-bx-1x15-flip-top.png" => 0x01030003,
        "cab-bx-1x15-jazz-tone.png" => 0x01030004,
        "cab-bx-1x18-session.png" => 0x01030005,
        "cab-bx-1x18-amp-360.png" => 0x01030006,
        "cab-bx-1x18-california.png" => 0x01030007,
        "cab-bx-1x18-12-stadium.png" => 0x01030008,
        "cab-bx-2x10-modern-uk.png" => 0x01030009,
        "cab-bx-2x15-doubleshow.png" => 0x0103000a,
        "cab-bx-2x15-california.png" => 0x0103000b,
        "cab-bx-2x15-class-a.png" => 0x0103000c,
        "cab-bx-4x10-tweed.png" => 0x0103000e,
        "cab-bx-4x10-adam-eve.png" => 0x0103000f,
        "cab-bx-4x10-silvercone.png" => 0x01030010,
        "cab-bx-4x10-session.png" => 0x01030011,
        "cab-bx-4x12-hiway.png" => 0x01030012,
        "cab-bx-4x12-green-20-s.png" => 0x01030013,
        "cab-bx-4x12-green-25-s.png" => 0x01030014,
        "cab-bx-4x15-big-boy.png" => 0x01030015,
        "cab-bx-8x10-classic.png" => 0x01030016,
    }
    Stomp {
        "stomp-blue-comp.png" => 0x02000002 | 0x02000003,
        "stomp-red-comp.png" => 0x02000004,
        "stomp-vetta-comp.png" => 0x02000005,
        "stomp-fx-vetta-juice.png" => 0x02000006,
        "stomp-auto-swell.png" => 0x02000007,
        "facial-fuzz.png" => 0x02050000,
        "stomp-fx-tube-drive.png" => 0x02050001,
        "stomp-fuzz-pi.png" => 0x02050002,
        "stomp-screamer.png" => 0x02050003,
        "stomp-octave-fuzz.png" => 0x02050004,
        "stomp-classic-dist.png" => 0x02050005,
        "stomp-fx-killer-z.png" => 0x02050006,
        "stomp-fx-boost-eq.png" => 0x02050007,
        "stomp-bass-overdrive.png" => 0x02050008,
        "stomp-bronze-master.png" => 0x02050009,
        "stomp-auto-wah.png" => 0x020a0000,
        "stomp-fx-synth-lead.png" => 0x020a0001,
        "stomp-fx-synth-string.png" => 0x020a0002,
        "stomp-fx-synth-analog.png" => 0x020a0003,
        "stomp-fx-synth-fx.png" => 0x020a0004,
        "stomp-fx-buzz-wave.png" => 0x020a0005,
        "stomp-fx-rez-synth.png" => 0x020a0006,
        "stomp-fx-saturn-5-ring-m.png" => 0x020a0007,
        "stomp-fx-double-bass.png" => 0x020a0008,
        "stomp-fx-synth-harmony.png" => 0x020a0009,
        "stomp-fx-dingo-tron.png" => 0x020a000a,
        "stomp-fx-clean-sweep.png" => 0x020a000b,
        "stomp-fx-seismik-synth.png" => 0x020a000c,
        "stomp-sub-octaves.png" => 0x020a000d,
        "stomp-bender.png" => 0x020a000e,
    }
    Mod {
        "mod-sine-chorus.png" => 0x02030004,
        "mod-analog-chorus.png" => 0x02030005,
        "mod-line-6-flanger.png" => 0x02030006,
        "mod-jet-flanger.png" => 0x02030007,
        "mod-phaser.png" => 0x02030008,
        "mod-u-vibe.png" => 0x02030009,
        "mod-opto-trem.png" => 0x0203000a,
        "mod-bias-trem.png" => 0x0203000b,
        "mod-rotarydrum-horn.png" => 0x0203000c,
        "mod-rotary-drum.png" => 0x0203000d,
        "mod-auto-pan.png" => 0x0203000e,
        "mod-fx-lumpy-phase.png" => 0x02030011,
        "mod-fx-square-chorus.png" => 0x02030012,
        "mod-fx-expo-flange.png" => 0x02030013,
        "mod-fx-random-chorus.png" => 0x02030014,
        "mod-fx-analog-square.png" => 0x02030015,
        "mod-fx-pod-purple-x.png" => 0x02030017,
        "mod-fx-random-s-h.png" => 0x02030018,
        "mod-fx-tape-eater.png" => 0x02030019,
        "mod-fx-hi-talk.png" => 0x0203001a,
        "mod-fx-sweeper.png" => 0x0203001b,
        "mod-fx-warble-matic.png" => 0x0203001c,
        "mod-fx-expo-chorus.png" => 0x0203001e,
        "mod-fx-square-flange.png" => 0x0203001f,
    }
    Delay {
        "analog-delay.png" => 0x02020002,
        "delay-analog-w-mod.png" => 0x02020003,
        "delay-tube-echo.png" => 0x02020004,
        "delay-multi-head.png" => 0x02020005,
        "delay-sweep-echo.png" => 0x02020006,
        "delay-digital-delay.png" => 0x02020007,
        "delay-stereo-delay.png" => 0x02020008,
        "delay-ping-pong.png" => 0x02020009,
        "delay-reverse.png" => 0x0202000a,
        "delay-fx-tape-echo.png" => 0x0202000b,
        "delay-fx-echo-platter.png" => 0x0202000c,
        "delay-fx-low-rez.png" => 0x0202000d,
        "delay-fx-phaze-eko.png" => 0x0202000e,
        "delay-fx-bubble-echo.png" => 0x0202000f,
    }
    Reverb {
        "reverb-lux-spring.png" => 0x02040002,
        "reverb-std-spring.png" => 0x02040003,
        "reverb-king-spring.png" => 0x02040004,
        "reverb-rack.png" => 0x02040005..=0x02040010,
    }
    Wah {
        "vetta-wah.png" => 0x02060000,
        "wah-fassel.png" => 0x02060003,
        "wah-weeper.png" => 0x02060004,
        "wah-chrome.png" => 0x02060005,
        "wah-chrome-custom.png" => 0x02060006,
        "wah-throaty.png" => 0x02060007,
        "wah-conductor.png" => 0x02060008,
        "wah-colorful.png" => 0x02060009,
    }
    Compressor {
        "compressor.png" => 0x020b0001,
    }
    Eq {
        "eq.png" => 0x020c0002,
    }
    Gate {
        "gate.png" => 0x020b0000,
    }
    Volume {
        "volume.png" => 0x02070000,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    use super::*;

    #[derive(Deserialize)]
    struct Catalog {
        catalog: Vec<CatalogModel>,
    }

    #[derive(Deserialize)]
    struct CatalogModel {
        block: String,
        id: u32,
    }

    #[test]
    fn exact_catalogue_and_fixed_block_keys_cover_the_embedded_artwork() {
        let catalog: Catalog =
            serde_json::from_str(include_str!("../../../research/podxt-live-parameters.json"))
                .unwrap();
        let mut keys = BTreeSet::from([FRAME_KEY]);
        assert_eq!(catalog.catalog.len(), 246);
        for model in catalog.catalog {
            let block = match model.block.as_str() {
                "amp" => Block::Amp,
                "cab" => Block::Cab,
                "wah" => Block::Wah,
                "stomp" => Block::Stomp,
                "mod" => Block::Mod,
                "delay" => Block::Delay,
                "reverb" => Block::Reverb,
                unknown => panic!("Unexpected catalogue block {unknown}"),
            };
            let key = model_key(block, Some(model.id));
            if model.id == 0x01020000 {
                assert_eq!(block, Block::Cab);
                assert_eq!(key, None);
            } else {
                keys.insert(key.unwrap_or_else(|| panic!("{block:?} {:#010x}", model.id)));
            }
            assert_eq!(model_key(block, None), None);
        }
        for (block, id, key) in [
            (Block::Amp, 0x00020000, "pod-farm/amp-no-amp.png"),
            (Block::Gate, 0x020b0000, "pod-farm/gate.png"),
            (Block::Compressor, 0x020b0001, "pod-farm/compressor.png"),
            (Block::Eq, 0x020c0002, "pod-farm/eq.png"),
            (Block::Volume, 0x02070000, "pod-farm/volume.png"),
            (Block::Stomp, 0x020a000e, "pod-farm/stomp-bender.png"),
            (Block::Mod, 0x02030004, "pod-farm/mod-sine-chorus.png"),
            (Block::Reverb, 0x02040002, "pod-farm/reverb-lux-spring.png"),
            (Block::Reverb, 0x02040010, "pod-farm/reverb-rack.png"),
        ] {
            assert_eq!(model_key(block, Some(id)), Some(key));
            keys.insert(key);
        }
        for (block, id) in [
            (Block::Volume, Some(0x020a000e)),
            (Block::Stomp, Some(0x02070000)),
            (Block::Delay, Some(0x02040002)),
            (Block::Wah, Some(0x02060002)),
            (Block::Amp, Some(0xffff_ffff)),
            (Block::Gate, None),
            (Block::Compressor, None),
            (Block::Eq, None),
            (Block::Volume, None),
        ] {
            assert_eq!(model_key(block, id), None, "{block:?} {id:?}");
        }

        let sources = include_str!("../../../assets/ui/SOURCES.md");
        let mut documented = BTreeSet::new();
        let ctx = egui::Context::default();
        // This test also runs directly from a copied test executable.
        for line in sources
            .lines()
            .filter(|line| line.starts_with("| `pod-farm/"))
        {
            let columns: Vec<_> = line.split('|').map(str::trim).collect();
            let key = columns[1].trim_matches('`');
            if [
                "pod-farm/volume-pedal.png",
                "pod-farm/sine-chorus.png",
                "pod-farm/lux-spring.png",
            ]
            .contains(&key)
            {
                continue; // The earlier prototype art is still used by the prototype/package.
            }
            assert!(documented.insert(key), "{key}");
            let expected_digest = columns[columns.len() - 2].trim_matches('`');
            let dimensions = columns[4].split(", ").nth(1).unwrap();
            let (width, height) = dimensions.split_once('×').unwrap();
            let expected_pixels = [
                width.parse::<usize>().unwrap(),
                height.parse::<usize>().unwrap(),
            ];
            let bytes = png(key).unwrap_or_else(|| panic!("{key}"));
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                expected_digest,
                "{key}"
            );
            let texture = load(&ctx, key);
            assert_eq!(texture.name, key);
            assert_eq!(texture.pixels, expected_pixels, "{key}");
            assert!(!texture.missing && texture.texture.is_some(), "{key}");
        }
        assert_eq!(keys, documented);
        assert_eq!(keys.len(), 237);

        for invalid in [
            "",
            "volume.png",
            "pod-farm/Volume.png",
            "pod-farm/../pod-farm/volume.png",
            "/pod-farm/volume.png",
            "pod-farm/unknown.png",
        ] {
            let image = load(&ctx, invalid);
            assert!(image.missing && image.texture.is_none(), "{invalid}");
        }
    }
}
