# PODxt stage artwork coverage

The PODxt editor uses the embedded POD Farm 2.58 stage images for stored model
IDs. Artwork selection is independent of packs and of the file, backup or
playing-tone origin. The model picker still applies the existing pack rules.
The signal chain keeps its ten blocks, route badges, bypass switch and model
captions. Its existing aspect-ratio rule also sizes the newly mapped images.
The Amp detail shows the cabinet icon beside the Cab selector where the row has
space. [Artwork sources and checksums](../../assets/ui/SOURCES.md)

| Block | Editor catalogue IDs | Without packs | Image coverage |
| --- | ---: | ---: | --- |
| Amp | 107 | 43 | All, including the original No Amp image |
| Cab | 47 | 25 | 46 in Amp detail, with No Cab intentionally blank |
| Wah | 8 | 8 | All |
| Stomp | 31 | 14 | All |
| Mod | 24 | 11 | All |
| Delay | 14 | 9 | All |
| Reverb | 15 | 15 | All |
| Gate | Fixed | Fixed | Noise Gate `0x020b0000` |
| Comp | Fixed | Fixed | Compressor `0x020b0001` |
| EQ | Fixed | Fixed | Four-band EQ `0x020c0002` |
| Volume | Fixed | Fixed | Volume Pedal `0x02070000` in either routing position |

## Drawn glyphs and blank cabinet preview

- Gate, Wah, Stomp, Amp, Comp, EQ, Volume, Mod, Delay and Reverb use their
  existing glyph when the editor has no unambiguous model in the appropriate
  slot or the ID has no exact artwork. This covers missing slots, duplicate
  candidates including both routing positions, foreign fixed IDs and unknown
  IDs. There is no additional selectable Off model for the effects.
- A bypassed block with a mapped model keeps its artwork with the existing
  gray tint. A bypassed block with an unavailable model keeps its dim glyph.
  Volume has no bypass switch. No Amp `0x00020000` has a dedicated image.
- No Cab `0x01020000` has image ID zero in the source table. Missing or
  unmapped cabinet IDs leave the preview blank as well. The Amp tile
  represents the amp model.
- Mic and A.I.R. remain controls in Amp detail without artwork previews.

The PODxt file asset-name lookup also recognizes the IDs below. They fall
outside the editor's 246 catalogue IDs and can be encountered as raw slots in
imported files. Their display names alone establish no exact stage image. These
are the named gaps, in addition to the general missing or unknown cases above.

| Block | Model IDs recognized by the file asset-name lookup | Reason |
| --- | --- | --- |
| Amp | `0x00000000` Tube Preamp | Earlier POD amp IDs. |
| Amp | `0x00000001` POD Clean | Earlier POD amp IDs. |
| Amp | `0x00000002` POD Crunch | Earlier POD amp IDs. |
| Amp | `0x00000003` POD Drive | Earlier POD amp IDs. |
| Amp | `0x00000004` POD Layer | Earlier POD amp IDs. |
| Amp | `0x00000005` Small Tweed | Earlier POD amp IDs. |
| Amp | `0x00000006` Tweed Blues | Earlier POD amp IDs. |
| Amp | `0x00000007` Black Panel | Earlier POD amp IDs. |
| Amp | `0x00000008` Modern Class A | Earlier POD amp IDs. |
| Amp | `0x00000009` Brit Class A | Earlier POD amp IDs. |
| Amp | `0x0000000a` Brit Blues | Earlier POD amp IDs. |
| Amp | `0x0000000b` Brit Classic | Earlier POD amp IDs. |
| Amp | `0x0000000c` Brit Hi Gain | Earlier POD amp IDs. |
| Amp | `0x0000000d` Treadplate | Earlier POD amp IDs. |
| Amp | `0x0000000e` Modern Hi Gain | Earlier POD amp IDs. |
| Amp | `0x0000000f` Fuzz Box | Earlier POD amp IDs. |
| Amp | `0x00000010` Jazz Clean | Earlier POD amp IDs. |
| Amp | `0x00000011` Boutique 1 | Earlier POD amp IDs. |
| Amp | `0x00000012` Boutique 2 | Earlier POD amp IDs. |
| Amp | `0x00000013` Brit Class A 2 | Earlier POD amp IDs. |
| Amp | `0x00000014` Brit Class A 3 | Earlier POD amp IDs. |
| Amp | `0x00000015` Small Tweed 2 | Earlier POD amp IDs. |
| Amp | `0x00000016` Black Panel 2 | Earlier POD amp IDs. |
| Amp | `0x00000017` Boutique 3 | Earlier POD amp IDs. |
| Amp | `0x00000018` Cali Crunch 1 | Earlier POD amp IDs. |
| Amp | `0x00000019` Cali Crunch 2 | Earlier POD amp IDs. |
| Amp | `0x0000001a` Treadplate 2 | Earlier POD amp IDs. |
| Amp | `0x0000001b` Modern Hi Gain 2 | Earlier POD amp IDs. |
| Amp | `0x0000001c` Line 6 Twang | Earlier POD amp IDs. |
| Amp | `0x0000001d` Line 6 Crunch | Earlier POD amp IDs. |
| Amp | `0x0000001e` Line 6 Blues | Earlier POD amp IDs. |
| Amp | `0x0000001f` Line 6 Insane | Earlier POD amp IDs. |
| Amp | `0x00030000` BX-Bypass | Bass bypass alias outside the editor catalogue. |
| Cab | `0x01000000` 1x8 Tweed Champ | Earlier POD cabinet IDs. |
| Cab | `0x01000001` 1x12 Tweed Deluxe | Earlier POD cabinet IDs. |
| Cab | `0x01000002` 1x12 AC-15 | Earlier POD cabinet IDs. |
| Cab | `0x01000003` 1x12 Blackface Deluxe | Earlier POD cabinet IDs. |
| Cab | `0x01000004` 1x12 Line 6 | Earlier POD cabinet IDs. |
| Cab | `0x01000005` 2x12 Blackface Twin | Earlier POD cabinet IDs. |
| Cab | `0x01000006` 2x12 AC-30 | Earlier POD cabinet IDs. |
| Cab | `0x01000007` 2x12 Chieftain | Earlier POD cabinet IDs. |
| Cab | `0x01000008` 2x12 Line 6 | Earlier POD cabinet IDs. |
| Cab | `0x01000009` 4x10 Bassman | Earlier POD cabinet IDs. |
| Cab | `0x0100000a` 4x10 Line 6 | Earlier POD cabinet IDs. |
| Cab | `0x0100000b` 4x12 Vintage 30s | Earlier POD cabinet IDs. |
| Cab | `0x0100000c` 4x12 Stock 70s | Earlier POD cabinet IDs. |
| Cab | `0x0100000d` 4x12 Greenbacks | Earlier POD cabinet IDs. |
| Cab | `0x0100000e` 4x12 Line 6 | Earlier POD cabinet IDs. |
| Cab | `0x0100000f` No Cabinet | Earlier POD cabinet IDs. |
| Cab | `0x01020017` 1x12 California | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x01020019` 2x12 Zen Master | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x0102001b` 4x12 Hiway | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x0102001c` 4x12 Higain Solo | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x0102001d` 2x10 Vibro | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x0102001e` 4x12 X-Load | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x0102001f` 4x12 Big Bottom | Named cabinet IDs outside the PODxt Live selector catalogue. |
| Cab | `0x01030000` BX-No Cabinet | Bass No Cabinet alias outside the editor catalogue. |
| Wah | `0x02060002` Vetta Wah | The similarly named Vetta Wah uses a different ID. Custom has no image entry. |
| Wah | `0x0206000a` Custom | The similarly named Vetta Wah uses a different ID. Custom has no image entry. |
| Stomp | `0x02000008` Auto Swell | Named legacy Auto Swell variant. |
| Mod | `0x02030000` Chorus 1 | Named alternate modulation IDs. |
| Mod | `0x02030001` Flanger 1 | Named alternate modulation IDs. |
| Mod | `0x02030002` Rotary | Named alternate modulation IDs. |
| Mod | `0x02030003` Tremolo | Named alternate modulation IDs. |
| Mod | `0x0203000f` Chorus 2 | Named alternate modulation IDs. |
| Mod | `0x02030010` Flanger 2 | Named alternate modulation IDs. |
| Mod | `0x02030016` Stereo Chorus | Named alternate modulation IDs. |
| Delay | `0x02020000` Delay | Named alternate delay IDs. |
| Delay | `0x02020001` PingPong | Named alternate delay IDs. |
| Delay | `0x02020010` Ducking Delay | Named alternate delay IDs. |
| Reverb | `0x02040000` Room | Named alternate reverb IDs. |
| Reverb | `0x02040001` Spring | Named alternate reverb IDs. |
| Reverb | `0x02040011` Large Room | Named alternate reverb IDs. |
| Reverb | `0x02040012` Studio 6 | Named alternate reverb IDs. |
| Reverb | `0x02040013` Memphis Plate | Named alternate reverb IDs. |
| Reverb | `0x02040014` Foil Plate | Named alternate reverb IDs. |
| Reverb | `0x02040015` Blue Plate | Named alternate reverb IDs. |
| Reverb | `0x02040016` Bingo Hall | Named alternate reverb IDs. |
| Reverb | `0x02040017` Concert Hall | Named alternate reverb IDs. |
| Reverb | `0x02040018` War Memorial | Named alternate reverb IDs. |
| Reverb | `0x02040019` Hangar 18 | Named alternate reverb IDs. |
| Reverb | `0x0204001a` Propellerverb | Named alternate reverb IDs. |
| Reverb | `0x0204001b` Radioverb | Named alternate reverb IDs. |
