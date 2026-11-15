# Original Workbench artwork and layout

Run `python scripts/catalog_ui_assets.py` to copy original raster assets unchanged
from the extracted Windows application into `assets/original-ui/`. This helper
uses Pillow for asset metadata and contact sheets; it is not part of the Rust app.
The copied images retain their original bytes, format, size, transparency and
multi-frame content. Previews are separate, resized images.

The catalog is `analysis/ui-assets/index.html`, with filtering by path and a
SHA-256/dimensions/frame-count manifest alongside it. It contains 396 raster
assets, including 141 in `modules/variax/editor/images/`. These counts include
controls, help illustrations and duplicate legacy resources, not just guitars.

Useful paths relative to `assets/original-ui/`:

| Artwork | Location |
| --- | --- |
| Guitar body close-ups (23 images, some shared by multiple models) | `modules/variax/editor/images/bodies/` |
| Body-selection thumbnails and switch-state sprites | `modules/variax/editor/images/bodies/buttons/` |
| Body wireframe diagrams | `modules/variax/editor/images/bodies/wireframe/` |
| Pickup artwork, unknown-model icon, movement/rotation hints | `modules/variax/editor/images/pickups/` |
| Fretboard used by tuning controls | `modules/variax/editor/images/fretboard.png` |
| Buttons, sliders, knob frames, panel backgrounds | `modules/variax/editor/images/` and its subdirectories |
| Separate acoustic editor resources | `modules/variaxacoustic/editor/images/` |
| Annotated original editor illustrations | `variaxworkbenchapp/images/help/editor1.gif` and `editor2.gif` |

Contact sheets: `analysis/ui-assets/guitar-bodies.png`, `pickups.png`, and
`neck-and-tuning.png`. These show original graphics, not a running reconstruction.
Some neck/fingerboard portions are baked into the body art; there is also a
separate fretboard graphic. A separate complete neck/headstock asset was not found
in the electric editor's top-level image directory.

## Layout recovered from Java

The original uses Java Swing components with fixed positions over bitmap panels.
`analysis/decompiled/mac/modules/variax/editor/L6VariaxEditor.java` contains:

* `Images` maps model IDs to body artwork, wireframes, selector thumbnails and
  pickup sprites. These mappings matter because multiple models share artwork.
* Main body artwork is added at `(12,132)` with a `426×412` area.
* Both movable pickup controls occupy an overlay region at `(20,267)`, `438×160`.
* Tuning controls use the separate `fretboard.png` image.
* The chrome knob image is loaded as a `4×12` array of frames; other controls
  likewise use image grids for states. These should not be treated as single icons.

`L6PickupControl.java` implements position and angle controls, transformed image
painting, mouse dragging, and disabled-pickup transparency (0.4 on Windows,
0.5 on Mac). The recovered layout and model mappings can inform a Rust UI without
running or embedding the Java application. Interaction fidelity remains untested.

The original assets and generated visual catalog are local, ignored artifacts.
No artwork has been redrawn or converted for the application at this stage.
