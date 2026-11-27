# UI artwork sources

The first table records the approved images from the layout and shell work.
The numeric POD Farm IDs are the XGIM index IDs. Installers, binaries and
complete archive extractions stay local.

| File | Product | Version | Archive path | SHA-256 |
| --- | --- | --- | --- | --- |
| `pod-farm/stage-frame.png` | POD Farm 2 | 2.58 | `res/App.imb` image `0x00010003`, 826×497, from `POD Farm 2/pkg-stage/POD_Farm_2_Common.pkg/unpacked/res/App.imb` | `bd99eb9de73e18654b37d7ede6284d50226ffd746233b3eba9ee085d7905d82f` |
| `variax/spank-body.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/strat.png` | `9f59a26b57c18840508f2e0b76af20ef9c2ef8fdaa405a8b5674fe511fe09e32` |
| `variax/spank-pickup.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/strat.png` | `b7155986abd2cc949562e0b05322bba7221a745d4be7e95f5cdc36bdb3cb4fb8` |
| `variax/fretboard.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/fretboard.png` | `aa322c45764fdc0da62bf83cd3b90babbb4c0184932264060ea642397cd9ff9c` |
| `shell/workbench-mark.png` | Variax Workbench | 1.75 | `variaxworkbenchapp/images/icon.png` | `c6605b76df17861cbff74eae4b446612218d7d2aed4ad18b7ef12f0120ee30c2` |
| `shell/podxt-live.png` | Variax Workbench | 1.75 | `cmn/l6java/resources/images/icons/products/64x64/PODxtLive.png` | `223f8d8a9b02c42aaa1542e06283f63a646bf3e126537dcfd0e0bb35bf32ef48` |
| `shell/variax.png` | Variax Workbench | 1.75 | `cmn/l6java/resources/images/icons/products/64x64/Variax.png` | `e282971a7c40ca0ed8b0ed5e62d6cdaf8b36c49825cac59dba7f390b2fbe779e` |
| `pod-farm/facial-fuzz.png` | POD Farm 2 | 2.58 | `res/FX.imb` image `0x00030088`, 174×171 | `0c74d8d8fe1c01bf6fc6c3f357b0853e096e6ae7b0021fa43278bd328b737a05` |
| `pod-farm/sine-chorus.png` | POD Farm 2 | 2.58 | `res/FX.imb` image `0x00030041`, 800×210 | `6dc470942e13a851ef3253a46525c4921dde20feffbbd4a1c2a052691a965354` |
| `pod-farm/analog-delay.png` | POD Farm 2 | 2.58 | `res/FX.imb` image `0x000300c0`, 119×143 | `3a5b3d564c505de126e2a970d731fbc5f7462ae0c27ec567b7a26319149d50cb` |
| `pod-farm/vetta-wah.png` | POD Farm 2 | 2.58 | `res/FX.imb` image `0x000300d2`, 137×204 | `87588e5e747caaabb4987507a0f775aee90a9921c7774ed9e3d4dd3510d3d63d` |
| `pod-farm/lux-spring.png` | POD Farm 2 | 2.58 | `res/FX.imb` image `0x00030049`, 800×210 | `c5ae41de85619b1592a4780e00895e1982776f6055430598a1ea7056d78f9a9b` |
| `pod-farm/volume-pedal.png` | POD Farm 2 | 2.58 | `res/FX.imb` image `0x0003009a`, 240×232 | `4d11fc387c47c5a438fd24423ddb6f463dca5ab1f737254a8b6dfb4c35d337fd` |

The Workbench paths are relative to the Windows Variax Workbench 1.75
extraction under `analysis/extracted/windows-app/`. The POD Farm paths are
relative to the POD Farm 2.58 Mac extraction under
`analysis/extracted/rea-full/pod-farm-2.58-mac/POD Farm 2/pkg-stage/POD_Farm_2_Common.pkg/unpacked/`.
Those trees are ignored.

`scripts/extract_xgim.py` splits an `.imb` file. The full App, Amp and FX
extractions stay ignored. The six FX images above are the POD Farm 2 skins
whose own lettering matches the example tone: Facial Fuzz, Sine Chorus,
Analog Delay, Vetta Wah, Lux Spring, and the volume pedal. No image in this
set was generated. The Variax neck in the prototype is drawn as vectors in
the page, so `variax/fretboard.png` is no longer placed on that page. Amp
faces in Amp.imb name other models (for example `64 Lux`), so the mockup
draws one shared amp panel and sets the model name in the page.

## Variax editor artwork (#224)

The Workbench executable embeds this shared set on Linux, macOS and Windows.
It resolves 23 stage body images for 28 body models, 15 stage pickup sprites
for 32 pickup models, 25 bank thumbnails and 25 Body inspector outlines.
The two Spank stage images use the previously approved entries above, with
runtime keys `bodies/strat.png` and `pickups/strat.png`.

The source is the Windows Variax Workbench 1.75 application archive
`Variax Workbench/Variax Workbench.exe` inside the installer. The corresponding
members in the legacy macOS 1.75 b121 `VariaxWorkbench.jar` have the same
bytes. The PNGs below were re-encoded from the decoded pixels without vendor
metadata. Their decoded RGBA pixels and native dimensions match the source,
including pickup sprite alpha used for hit testing.

Each new file uses its path below `variax/` as the runtime key.

| File | Product | Version | Archive member and dimensions | Source SHA-256 | Repackaged SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `variax/bodies/acoustic.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/acoustic.png`, 426×412 | `ffcffaa08ae9ca7782baf196c603e275eae49a81392b24af461ddfd3e2e61764` | `bd522152cab267e5322a4b77722b4b4762f03a163403990dcd4a55622b7c800d` |
| `variax/bodies/banjo.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/banjo.png`, 426×412 | `65e1e0c41d966a20584e50ce89e4c9bb49a7351a6912ef7c56e61db31da466a9` | `ab9833af892a79104bb3765e3ee637eec7f927a6d4f1e445c7db55f1ec1be911` |
| `variax/bodies/casino.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/casino.png`, 426×412 | `c8e5229f0983d860dbafa1a6e28e03d52314d53170ace390d2541b4ee84f8abb` | `20fe6a5d1affb5f5c8d84490b54099ec6b28511d4514566ed46f2d27bb006275` |
| `variax/bodies/countrygent.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/countrygent.png`, 426×412 | `9e97b2db8ac1536532f275d48f581bef68a33d9ccf6f3a9328973729ca257866` | `1978b4ca43f11c8d5a7368e2fb872ebc1fd55828698ee69df78909b8e3f90a2d` |
| `variax/bodies/danelectro.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/danelectro.png`, 426×412 | `977995e6a591abede9e18b353a2ad729963a5a318e93994eb6e0d2973ba81552` | `3de4357b954264ef4b9cf951d11cc3ad50a3886efbb2aab232b19f256d84d1e1` |
| `variax/bodies/dobro.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/dobro.png`, 426×412 | `379c5583a7966cbfd237572bc648f86655a0c30a3495f3dcc6694003d8fbb59c` | `2f1fea0d31a55c32a39475e3ed2396696dbe15ccefb68d58f2f04563fa94ec7f` |
| `variax/bodies/es175.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/es175.png`, 426×412 | `9af43797a7bd76c51da1e0c37325f9018130915dceaf86e88a6f76c99339f829` | `238d6710858a367f80c6f29ec521771e508981b731e573be852965fa17ca6ef3` |
| `variax/bodies/es335.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/es335.png`, 426×412 | `a1640d40dc59422b1325a162b57de334ff3401265493eab0e03e36f321d416ec` | `b3b5199fec6663d048704166a600f2fd8192534f760f8894c8cd09d60fb37d75` |
| `variax/bodies/firebird.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/firebird.png`, 426×412 | `a698eac90936303d39ec8758282e170ba0ef2b729c751e55568dbdaf378975f6` | `441c7b67af63bd65ef6bf9d98a62ea16f6101d40e4b5d65b450111230b88f152` |
| `variax/bodies/lester.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/lester.png`, 426×412 | `f13ac47ea2fcc056f2f360b570b3b6761110d3ed41e9b46c0c47d9d6838a8881` | `cd5d7db797fbdf9faf866c5aa1464a02d7d6890a51dfac9bcd871270170fcc67` |
| `variax/bodies/lesterblack.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/lesterblack.png`, 426×412 | `482bbf25a8675807a5db714c4c33623c125540fc4e462ee1615cc4d341871cde` | `709113fad48dd252d95d47d3f35ca61a3a3eca3a55828b1252559012e4b7f707` |
| `variax/bodies/lestergoldtop.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/lestergoldtop.png`, 426×412 | `ed525cf53f91732005f34e59c4c522189eb3377ec17d2d7b5781d72e7483b371` | `e5114191f2b7538a6e24bbf67209fb6dd26584bc6dd1ffe625794807812d1c1f` |
| `variax/bodies/lpjunior.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/lpjunior.png`, 426×412 | `93b7169091e00f8af54fc9b215644f585946dde4a0d963225c68d90415e6e1b8` | `0ed7a8eb3ef73e5965512d5d14cc591712164c71a8ecaee0cca8a04b66f905a1` |
| `variax/bodies/ricky.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/ricky.png`, 426×412 | `c4c6b9a52660d1e4ee2ad04ab67a1a41302e958a42cc2e33b3096bc0331271f9` | `3510f4803449690e451848f0a10cc5ef71704b0e8757e31d3a6b5a6213a8b8f9` |
| `variax/bodies/sitar.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/sitar.png`, 426×412 | `39ff7b60bc14a466bb542208cf57aa464b6c75dbcced4742227f0678ca5fddc4` | `2a1c56a11659725ab46304a293f9bf0193218fd2b180575b9added9f3558be9a` |
| `variax/bodies/sparklejet.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/sparklejet.png`, 426×412 | `b16f69f67c7ac89a9b95b9d5958460e0158f0e8871e8fb32faf646bfb81ca26b` | `30ff2453b68cd6cf713aff670bf8e3e9270a9de0eef3b012602be6837e376aa3` |
| `variax/bodies/special.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/special.png`, 426×412 | `d8fad0c43196958939a215d732c1610a02d48a5e65806fb731223678be4ac6d9` | `0968a968364055557c23edc5596726ae12835e5b97a7e4692c0c3ee7cfe4606a` |
| `variax/bodies/super400.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/super400.png`, 426×412 | `0a9d9f472d4ce964aa5e683055772af6e55d6df79bdbae1271d1bf0f2e4862e3` | `475824ac08177464225c598ad1a95735db5a080da7a5c267edf6f6c81d5fc6c6` |
| `variax/bodies/tele.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/tele.png`, 426×412 | `594975550452901ae7347fb49f1945706f4b2ccd39fa4aabbb808cc4389426dd` | `04a3b5382dbf1cb21d376125c1a4716bb9f56d350739ad7d8f177846f7322a0a` |
| `variax/bodies/telecustom.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/telecustom.png`, 426×412 | `d5bff6db7961d417230d9a83fd1a76bbd0876fab5829e5c54df6b1c210d326c0` | `ffdaba455adba97992b0ce08b892d5fec85813ce8a39e171474299c48ea57526` |
| `variax/bodies/thinline.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/thinline.png`, 426×412 | `4bd97e3b96bf0ba9cf67e4d8760a96a77496cb158fa8c6c2dfed3ed03580f336` | `1d86d2cd308c8c80e4dc435bddad7f4fe4c0ca826834dc3172b739b1683c8a9f` |
| `variax/bodies/tricone.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/tricone.png`, 426×412 | `2619fae8fc3f4d6b40affe08d23812879d3a406ffbc396c63d497dbc24c9fb0a` | `2e78eb62c8a572d66beded166fa3e0087cf585c000c327d95571964f28430ce0` |
| `variax/bodies/wireframe/banjo.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/banjo.png`, 238×148 | `a043233b1d2c7aa693e7004fee085960709f88a445f6c1bf1df75800dba654cb` | `d1d662808c5e48d644dbb66f92d2e0661b0a58dfa87e4b620bbd6e5a6d71aaa4` |
| `variax/bodies/wireframe/casino.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/casino.png`, 238×148 | `dab4153b6f44d30d44cca4fd290833a86dbf33fc1669e51129f5dd5d4aa8a547` | `f51f83febedf38783338629acbf02c5a9814a639a30ee642eab8bb6f4e8d414d` |
| `variax/bodies/wireframe/danelectro.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/danelectro.png`, 238×148 | `a9168a0017b575ca2a06ec1bba6e29091f5c148b92310a96b49a5129790bd5eb` | `1b990c6fc76f4a098b6d6fdd3bdaac3783202230205e5fa8733ab3c230fbed9f` |
| `variax/bodies/wireframe/dobro.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/dobro.png`, 238×148 | `ec737835fae837ea98a1f86dd337f51413cd2a1f5d9843605edeee7db6764625` | `881b82a58f8512feacebbb0f6c732cb517f2677103b54e9f58373ec8c98e5bf3` |
| `variax/bodies/wireframe/es175.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/es175.png`, 238×148 | `f1ac9a12f2c374e1111fb9b5041f6acd656f5d9c6068f90cadb6859f6bb26205` | `86a01dcd69b037154fd2186e67ca55a9760c53b19ae184707ab9cf2e222a4d45` |
| `variax/bodies/wireframe/es335.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/es335.png`, 238×148 | `c3bfaa2fb2bb1083758f8167c8404cf1b503a61a4f6e62c13ad5be1692f0cbb5` | `986900d1a1cf7ef7fdcfbf5658f975ca7159cfbae3bf14708725592a13251497` |
| `variax/bodies/wireframe/firebird.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/firebird.png`, 238×148 | `c0531a106a227ab0a09fc755a07bd24f53d583153276a39c5bba11205ae197a8` | `30ec5c6f9a5a8ca8716f8e231cbd7874c103c214a98ed3e2bea88e4e59fed293` |
| `variax/bodies/wireframe/gretsch6120.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/gretsch6120.png`, 238×148 | `70d4553fcff0c27d84956d67a0c8e1e666b24ec6129d362e064aed39ed3bc72e` | `660e17465977554263b1e7856c86d8caadab25568da1962c43c99b43c8b9158a` |
| `variax/bodies/wireframe/guildf212.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/guildf212.png`, 238×148 | `3d1a3d0162c76c892e6f27bfec683fac0d27a6b01076c6995a5f446ae43abfdf` | `eb7efefa0afe3ab8d2759cacbd79e73236030078db5e5ac1d72bce7d72d6379f` |
| `variax/bodies/wireframe/j200.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/j200.png`, 238×148 | `e2e6c64a47555a0d9f32630640bed3010a4264fc829596cb9947d89815ba296f` | `a0306b56c89029e7f717bd4bdf6730192d85b433551a4873f8c1f42b4d338562` |
| `variax/bodies/wireframe/juniorsinglecut.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/juniorsinglecut.png`, 238×148 | `5dad0cf6776a4e33a1d38ae3da4f74727d608f9c595fa7ce7d1425fbab71302b` | `4bd30ab75084038320082a3338d58a3f3bd2f19a885e77b7bb67a45c84b030f0` |
| `variax/bodies/wireframe/lespaul.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/lespaul.png`, 238×148 | `7da1252bd00586af74731c63b208f1be073a29f9055ad1ff0104f5acd05b05d3` | `a4e0699c58c1afb7b52a80e58e8edc61ca3ce8b0d1676a36705155ccc4a5e901` |
| `variax/bodies/wireframe/lpspecial.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/lpspecial.png`, 238×148 | `a96907b019504b9e938226e8ffe1919480d882e0ae7a3831aa79606627a3b9be` | `770b2edd8258a13771ddbf429131b8337b8df991e64f1208bea9382eb177c883` |
| `variax/bodies/wireframe/martin018.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/martin018.png`, 238×148 | `89eda4ae90693ff5b1bc524d58586d46a403f2fa665e1c1e3a64029334321397` | `9265532f5bc0bffb5c44fec89dccbee4dcca773b4c9971d8a534e1f46d927c78` |
| `variax/bodies/wireframe/martind12_28.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/martind12_28.png`, 238×148 | `d4608853d86e0f518e66043b22512d1bb963eada1169880358f2f4b83ab1e008` | `cde743e5ebb3280b9883d79a9e11e7c397ba242baaf03597468fb3a953e6a237` |
| `variax/bodies/wireframe/martind28.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/martind28.png`, 238×148 | `c732bfbaddf69da068bc0262970e6a2d5453de7f14f6b53bd247477196ab99e7` | `a9697e29a7d69985d62761a5d70608d8bbcbfbe706baaa9075740412bb7318f4` |
| `variax/bodies/wireframe/ric12.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/ric12.png`, 238×148 | `785e7d22ae9d0586e64df570df2b80e4c0b81329dd9bd850824e1561a3f0fa26` | `f32edfe994d8e416963fbb2d9b25107904b3bb8f1df1ff5b715dc81289bb584c` |
| `variax/bodies/wireframe/ric6.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/ric6.png`, 238×148 | `0dda255db940897134221b5c6e8f919b7a45290dc6ca10563ef0669dec56e306` | `08c77ee9aca805a314e02c860b1df9c8ee72e93acef1cd82f44f34b3f5142c1e` |
| `variax/bodies/wireframe/sitar.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/sitar.png`, 238×148 | `ea54c1d523a291d2a03219cc61eda7c910cf8be23f0d6ccd5676169600af28b0` | `c957d042f74447ec215d8e90dd424f337b028fa08b1f04953b460649a220dbfd` |
| `variax/bodies/wireframe/sparklejet.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/sparklejet.png`, 238×148 | `081855639a51f5e26046d08156966530628213d3d2e3dae09359b377969d1a34` | `c5f39fc1beb5c8d1307a5a89c83e952a9b05d29508976cf915bb436ae60431f1` |
| `variax/bodies/wireframe/strat.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/strat.png`, 238×148 | `dce7f890f358dd44419bb355bed83991980a66ffbf7792c7856f333d1437b21c` | `46e96a222c78d7deae8e90d2a14c2ace0fa03c025e0e99dce14a28a4ebbae28a` |
| `variax/bodies/wireframe/super400.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/super400.png`, 238×148 | `9c344bffcbd6bf7c4cc8b9422694975a9a78a5e40dd66d44b2929355ff19bb76` | `481dc2a2dac7b2553dc9ac077b30abc6e372060ffe95b3da2f4334c98a62ebb2` |
| `variax/bodies/wireframe/tele.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/tele.png`, 238×148 | `2e4be8ce0a03cee9a739236ab1c848ffab50e1b5962941bec61e956e0f4868b0` | `f3b55ba63f1f168f59422900b0ed1608d707900beeda7bfdbbc19603e78aa03c` |
| `variax/bodies/wireframe/thinline.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/thinline.png`, 238×148 | `b5b0766ef22a69c9cd4a4d35351a4cb8eda1212426fc0a667b28496bc0903be8` | `83f1716a080e846faee36101a16b2460393402516e8199aac401c5189d72d370` |
| `variax/bodies/wireframe/tricone.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/wireframe/tricone.png`, 238×148 | `50a3a7a39c584a16f6c6db7b1b0ee53fd7d067fb19be38d217cb03dedf63cece` | `790436a43bbdbf7b1bfde52c2f72e84664a53327372675e57b224d85d8974202` |
| `variax/pickups/blackp90.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/blackp90.png`, 120×120 | `b17e8352abe43bc3f990eeace2f3ca50669131a54355f2eb08394499984f93ee` | `3933d96d702c1e7b44962bb0c9ccfeaf9918d73fd776d09530e86ce2509dbea4` |
| `variax/pickups/chromep90.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/chromep90.png`, 56×130 | `b0159ebb2f41793d866c3f67de02551fe0e097f926a134cd4edb7903063b5173` | `8812b0758d278d438097c5fa5c974905f65d5ebe9e82eb335e8452b47ebf722e` |
| `variax/pickups/chromepaf_b.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/chromepaf_b.png`, 120×120 | `144ba2e61a94bb69313aa007dac173c6f9ca49188fa266d5f9208906b8f6a720` | `9dd9d419f613453bae50b733783e5e4274d63830b66bdbcb0ec7203fefb004e4` |
| `variax/pickups/chromepaf_n.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/chromepaf_n.png`, 120×120 | `a680f780f316d33877be88959ebae0941b9efd49e2ce96b150758080446bf06b` | `62061d12b4d8d448452d50cb62e3a8c92efe5b992ec036e2ef831cfb27154bfd` |
| `variax/pickups/cremep90.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/cremep90.png`, 120×120 | `5e47d1517ecaa13c8ef23979703556b1fec993bfe4a0a323f1ef062565064530` | `999e4cdc219e1567ce5dbd041210d3389092bb531858918a0affbd61613ec597` |
| `variax/pickups/dearmond.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/dearmond.png`, 120×120 | `38ad3990e26d2cc2efa3d45c64240fdeb199d64847b1162801ffebbea38e48cc` | `090772b9734ea4f7055c6f0889f2f5e8bb85c879c9247ae47efab5efe2e213c7` |
| `variax/pickups/filtertron.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/filtertron.png`, 120×120 | `5b4ff87a933a693727e5fec109d54c812e34789d047eae50a81505790588f578` | `2c313524a5cd4d471a553c8627ef720fc2df8772b8f9ad25ce09cdd0a9fd6605` |
| `variax/pickups/goldpaf_b.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/goldpaf_b.png`, 120×120 | `8df851e7406f3b678f2fbfb1c09f5022edbbf64685ec8d069964e384ffd5d627` | `c114e2cddd63fbbb564f4106684ed81c854f7cee4024e238cabffd142f4625f4` |
| `variax/pickups/goldpaf_n.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/goldpaf_n.png`, 120×120 | `8b87d0bb39f7f92bd8002d7708c7b468cf7da8fab260660227e51c08d8a3d29b` | `61cdcf7d4a5b90b91976bb863ea028ccc41c372742865509cca23587fa6bd192` |
| `variax/pickups/lipstick.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/lipstick.png`, 120×120 | `d2b2594a0e961875a25f3961797cd4ed5c95e630740b9f967de9273322ec5474` | `5883c3bf243966732ddf89db1a4b6e818c082fa89d5b10568a7d089dc6f5df66` |
| `variax/pickups/mini.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/mini.png`, 120×120 | `228a35f1cc12dd6312858c34ebffd9a051af8a757252e8d61897cef3011fec12` | `fc4f0cc468ad769d77d11f3f8a451e04f15176d3ae140e6b78fd0db01cdee4ca` |
| `variax/pickups/telebridge.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/telebridge.png`, 120×120 | `4f27dd22cdb5f89d36cd1a28f90512f9fc874976f1c86bfe72c29c83497102b0` | `52f03dc6958e2b3d77ace69ae012aecfeeeb2af672919a652b4d38fd7fab702b` |
| `variax/pickups/teleneck.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/teleneck.png`, 120×120 | `d5892a693024ea49fc24fa3c5a6fb7c832f7cb2be5cd940dd3eb50e9d6bee89c` | `26cb2a5e455b12b9dea01c23e8d70b30343cf320a9a680351512552ba957a572` |
| `variax/pickups/toaster.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/pickups/toaster.png`, 120×120 | `c7521870e50e865f5f32fdf887e0869f72f4f923c9f89758040f337f0e620e8a` | `5e2b68fae980fb36e190e513ef6f7bdcfd63a12e62aa2fda5e272c26f279ccec` |
| `variax/bodies/buttons/banjo.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/banjo.png`, 42×50 | `2ce2f933b19e60a049611d2d4f01ee6fa9b77424d0a9bd2518e61cbdaae9979e` | `b9f8622ecaa3bcae5cc29e6f8423df8276b507d63fb076f6ab828ad4ffe482b5` |
| `variax/bodies/buttons/casino.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/casino.png`, 42×50 | `2f3c5dfd8636a36f241fcba5f293019176ba9c9f69b0b736bcd98256d48a70c1` | `77ae9973602e21150023b6c1a1de8386454463623b7ea07e220a3c2795d30673` |
| `variax/bodies/buttons/danelectro.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/danelectro.png`, 42×50 | `dd339bd60404212f9208333b3901ac4d44420a3dc48ff4b54e1622b5f3d077a9` | `6358d24c10341a61f5c503f941b15fdc6716ced35a5b8a9f00508b3b0c2d0fb0` |
| `variax/bodies/buttons/dobro.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/dobro.png`, 42×50 | `93b6b0e581301b66d106ab5e72b801e17d940c1ac3afb1b5edfeeef0c47856f8` | `65d4311e676eba4303e70bc5d91f06d31991e6dba77656def10acf18ee2d12c1` |
| `variax/bodies/buttons/es175.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/es175.png`, 42×50 | `3e4cbe45f2754ee56014a697d510178ab64a91283d03f8e658cc706dbdeb6070` | `657ffd61d28addb34588391ee0d471c5920e8d5a8ddcf38f1b04cc83a1671ce5` |
| `variax/bodies/buttons/es335.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/es335.png`, 42×50 | `cb2dffebe007790158ca2a8ba6d4af4e9b90bae8ba273075b075ac3ae8fc6ca2` | `2d84ab2a848fc07d5e4519bc1e6e4f4f0697151fe9db5646b3de5a682d103ed2` |
| `variax/bodies/buttons/firebird.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/firebird.png`, 42×50 | `0f837151d504588430ca14f64dadfd83574971028c4f103c3d45567e245100e2` | `9ddeef12dabd29e9ff9e36ba7bc9fc4d89ad003646e80ca0fc32342ec670a01b` |
| `variax/bodies/buttons/gretsch6120.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/gretsch6120.png`, 42×50 | `77ab8a83e0b3f9e084343503a4151486a4ffb347a48d1a690108ec1ea9362c6f` | `429f97203d772a5387529a74a55d11548d91300412261701660d804344ab08a8` |
| `variax/bodies/buttons/guildf212.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/guildf212.png`, 42×50 | `b9c9da5d037c63e7296adab1c218856e0eceb6f16d474a1e81bd26a9c45c7e81` | `e2854c17e010e740f4f950e955db24071fba96918c4b67cae148493b19f78a7c` |
| `variax/bodies/buttons/j200.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/j200.png`, 42×50 | `bc250954cd85346d3212e9c82183c3e95fafd7817644b8158fe915a919af1017` | `19fb3494608f85bc9b726dc38377167e6ddfa5980b555db0c9bfc33d3d0edd53` |
| `variax/bodies/buttons/juniorsinglecut.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/juniorsinglecut.png`, 42×50 | `dcc268aa3f21263114c8ea52c84029b28b94918a61a90bb2f4c03e2ab38a0a2c` | `5f74e6f66c570a45520792a2326401a2b7c325168dab470cd40f5543304bb38d` |
| `variax/bodies/buttons/lespaul.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/lespaul.png`, 42×50 | `bfe198ba86be64cdda2f3385cf00c9e2d0db1c43ab406d3f2f3043fc8ceab5b1` | `34bb0eec65f82569160d6597475b0baba4c7f1a823326d24896265521f04a0b3` |
| `variax/bodies/buttons/lpspecial.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/lpspecial.png`, 42×50 | `cc33cf5c12837c08409eb51ccf1ab7e97485012ffe3ddac3fdc132217b57f6c3` | `2d73ac9db661c60c5156b73d9f3c76c22ef9aa1cac43370a528823bd7ff2c8c0` |
| `variax/bodies/buttons/martin018.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/martin018.png`, 42×50 | `6956f21c4a922c2325284784bd11ff23a89851ba58aee203b3b14509b9696a0f` | `1fc372b9125bd860803e317737e02ecfddce50fe7fcfb62d38231ff50579ff80` |
| `variax/bodies/buttons/martind12_28.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/martind12_28.png`, 42×50 | `c32644786b6eb6ab5ad26d066798855388f46ddcd0e87894959e25797ce90a9f` | `ba6dc9d8f6de95dd52cda26550544c5d0964985d329796cabaec4d74ef7b02bd` |
| `variax/bodies/buttons/martind28.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/martind28.png`, 42×50 | `499557390401f6f319a058f4f647fa80603ee5ddb121218273b9f1098d232e68` | `844ac8a327ca1752d0f8f9d42c4bbc6e76aa094f963f4249958dd846728b0741` |
| `variax/bodies/buttons/ric12.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/ric12.png`, 42×50 | `6c8fd870da704437cf69d5cbf8417574bc3f1172b977cfa0582ecb42981cd56d` | `a2bf73058317a3dc1a2a156f3c55ece5709d1ef6a954565166044d978d504413` |
| `variax/bodies/buttons/ric6.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/ric6.png`, 42×50 | `f01743cefebf8781b14a7e6864d50779a92a3f96357c26b1fdd7fb120ef4f6eb` | `0e9b239a4f179c9a1d51abfcdb5d9cc22d1df10314bd2bc8e04236f70f7ecf2c` |
| `variax/bodies/buttons/sitar.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/sitar.png`, 42×50 | `a0e957ba241d9d6c142ab0330c7502251b3342cc99ff67ceaabd1f58113ef11e` | `a75698e2b6322098a86af0992b08e27bd0c4a944db7f2128c24cdd566447fcaa` |
| `variax/bodies/buttons/sparklejet.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/sparklejet.png`, 42×50 | `f2b02e4bbf016c157cb350e3e8d2e6d8d55c3b9ddc4d893ac2e3cf053016dac3` | `235e09f69f8eb3cee7e9dd6ff019647cabbd8b1af676cc3f16ab0eaeff51360f` |
| `variax/bodies/buttons/strat.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/strat.png`, 42×50 | `cb45b2298eaf9d39bdb57af82494f61c3f118f8f6d4a4dbdc1a4c74d77eb48e6` | `aa3ba58a2f64a37fbdf01b77954df90cb6998547872d4980a61d7ef2cd994e3a` |
| `variax/bodies/buttons/super400.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/super400.png`, 42×50 | `8d7964ab8d024b2bc14d7ddd2f1518a9277b86d3ccdff9bff746b66b2eb8127d` | `3c24fcfffd163c5705d4b032cee66a089e90002f1193ddf88bf22d5ad2f0c779` |
| `variax/bodies/buttons/tele.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/tele.png`, 42×50 | `94a843f12eab19746c62d4cf3d55f7e0424ca751291ec26efc342cc2d8c24363` | `8bedfa9418825f7c14bdc36a7f6d9e5208f67e26d49a72bd8af4b1ee8b7cc101` |
| `variax/bodies/buttons/thinline.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/thinline.png`, 42×50 | `dfc34ca991a99f53860b4ff5f2afa474b19163be1c38f652ecbe0177cb03e4ce` | `d2ea8094555fee416136a2450971d66017c55d53edb9462b79c42acb252d7af8` |
| `variax/bodies/buttons/tricone.png` | Variax Workbench | 1.75 | `modules/variax/editor/images/bodies/buttons/tricone.png`, 42×50 | `ab7fc7c3b3e508dc7d2da237e22fcf8f45fe481c6edf0de19de49119d61ea417` | `4b30ce92beb2a021a570c87df22b7aa5d27c88e229cb815f227fc271a6ff39ff` |
