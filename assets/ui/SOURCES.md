# UI art used by the layout mockups

These are the only images copied for issue #215. Each file is the original
image, unchanged. Installers, binaries and the rest of each archive stay
ignored. The numeric POD Farm ids are the XGIM index ids, not file names.

| File | Product | Version | Archive path | SHA-256 |
| --- | --- | --- | --- | --- |
| `pod-farm/stage-frame.png` | POD Farm 2 | 2.58 | `res/App.imb` image `0x00010003`, 826×497, from `POD Farm 2/pkg-stage/POD_Farm_2_Common.pkg/unpacked/res/App.imb` | `bd99eb9de73e18654b37d7ede6284d50226ffd746233b3eba9ee085d7905d82f` |
| `variax/spank-body.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/strat.png` | `9f59a26b57c18840508f2e0b76af20ef9c2ef8fdaa405a8b5674fe511fe09e32` |
| `variax/spank-pickup.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/strat.png` | `b7155986abd2cc949562e0b05322bba7221a745d4be7e95f5cdc36bdb3cb4fb8` |
| `variax/fretboard.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/fretboard.png` | `aa322c45764fdc0da62bf83cd3b90babbb4c0184932264060ea642397cd9ff9c` |
| `shell/workbench-mark.png` | Variax Workbench | 1.75 | `variaxworkbenchapp/images/icon.png` | `c6605b76df17861cbff74eae4b446612218d7d2aed4ad18b7ef12f0120ee30c2` |
| `shell/podxt-live.png` | Variax Workbench | 1.75 | `cmn/l6java/resources/images/icons/products/64x64/PODxtLive.png` | `223f8d8a9b02c42aaa1542e06283f63a646bf3e126537dcfd0e0bb35bf32ef48` |
| `shell/variax.png` | Variax Workbench | 1.75 | `cmn/l6java/resources/images/icons/products/64x64/Variax.png` | `e282971a7c40ca0ed8b0ed5e62d6cdaf8b36c49825cac59dba7f390b2fbe779e` |

The Workbench paths are relative to the Windows Variax Workbench 1.75
extraction under `analysis/extracted/windows-app/`. The POD Farm path is
relative to the POD Farm 2.58 Mac extraction under
`analysis/extracted/rea-full/pod-farm-2.58-mac/`. Those trees are ignored.

`scripts/extract_xgim.py` splits an `.imb` file. The full App, Amp, FX and
AppMojo extractions stay in the ignored `analysis/pod-farm-2.58-xgim/`
directory. Amp and FX images are model art and are not used by the mockups.
