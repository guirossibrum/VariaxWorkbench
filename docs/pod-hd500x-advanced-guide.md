# POD HD500X: bounded Advanced Guide evidence

Partial primary-document research for [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32).
This advances documented capability evidence only. No backend, codec, numeric
model/effect identity, driver negotiation or hardware support is implemented.

## Source and preservation

[Official Advanced Guide Rev A](https://line6.com/data/6/0a06434c6fb051e03e8ab63dc/application/pdf/POD%20HD500X%20Advanced%20Guide%20-%20English%20(%20Rev%20A%20).pdf)
returned HTTP 200, `application/pdf`, without redirect, authentication or cookies.
HTTP Date: `Mon, 16 Nov 2026 16:51:15 GMT`; retrieval completed
`2026-11-16T16:51:16.931371+00:00`. Size: **6,661,844 bytes**. SHA-256:
`bbfd4735191cff0dec2efa83d045a6fdd6f1dac9921f590d0f1f1df96aff0941`.
The cover identifies POD HD500X, Rev A, Electrophonic Limited Edition, ©2013.
Printed p. 1•1 scopes coverage to flash memory 1.01 or later; this is neither
an installed-version observation nor proof of every later firmware's behavior.
No publisher signature was authenticated.

The unchanged PDF was atomically published using a temporary file, fsync and
non-overwriting hard link in ignored `analysis/issue-32-advanced-guide/` as
`advanced-guide-rev-a.pdf`; `provenance.json` records retrieval and identity.
`pdftotext -layout` produced private `guide.txt`. No PDF, image, screenshot,
font, extracted text or vendor code is committed. Citations below give printed
pages; parenthetical PDF numbers are one-based pages including front matter.
The relevant connection, control, library and USB sections were read statically;
no vendor instructions were executed.

## Evidence matrix

Every row is a **documented capability or UI statement**, not observed runtime
behavior. Labels and human-readable numbers below are not stored/wire IDs.

| Primary pages | Documented observation | Boundary / unresolved evidence |
| --- | --- | --- |
| 1•1, 3•1, 3•3, 3•13 (PDF 5, 29, 31, 41) | Pre/post and parallel A/B arrangement; eight FX blocks; one or two amp blocks subject to placement and DSP limits. Mixer follows parallel paths. | No block serialization, DSP budget algorithm, model ID or parameter domain recovered. No routing emulator implemented. |
| 1•5, 4•1, 4•4–4•6 (PDF 9, 43, 46–48) | Edit is described as a Mac/Windows librarian. Device storage has eight setlists of 64 presets; banks expose A–D channels. Presets retain processing, assignment and relevant setup settings; unsaved state is displayed. | Storage capacity and UI addressing do not define H5EP/H5ES offsets, file product identity, checksums or unknown-field semantics. No Edit installer/oracle obtained. |
| 2•3–2•5 (PDF 13–15) | Two source selections feed processing; input setup can be preset-specific or global. Guitar impedance is a separate physical-input setting. | No raw enum values or stored encoding recovered. Global/preset selection is not a USB stream selector. |
| 9•1–9•2 (PDF 93–94) | Vendor driver supports Mac/Windows USB audio; processed recording signal goes to host, playback returns, and monitoring is mixed with playback at analog outputs. Guide specifies direct USB 2.0 connection and says DT25/DT50 L6 LINK use mutes USB audio. | Historical documented behavior, no Linux support/endpoint descriptors or current negotiated stream established. No connection performed. |
| 9•2, 9•5, 9•13 (PDF 94, 97, 105) | One stereo recording feed is named **1-2 Main Out**. Processing/mixer levels affect it; master volume does not. Hardware-monitor level is separate from recorded-send level. Windows page separately describes send-level boost. | No clean/reamp route is established by this chapter. Do not import PODxt/GearBox route enums or infer extra USB channels. |
| 9•4, 9•7, 9•12 (PDF 96, 99, 104) | Native processing rate is 48 kHz; guide describes 44.1/88.2/96 kHz through internal conversion. Mac format discussion fixes depth at 24 bits; Windows ASIO discussion recommends 24-bit. | Documented driver options, not observed supported-rate descriptors or byte packing. Windows recommendation is not proof of an exclusive bit-depth constraint. |

## Instrument distinctions

Printed pp. 2•4 and 2•11–2•16 (PDF 14, 21–26) distinguish JTV,
first-generation electric Variax, Acoustic 700 and Bass 700/705 controls over VDI.
JTV source selection distinguishes modeled/magnetic behavior; first-generation
instruments are directed to the Variax source. Local-control locking is described
for JTV but unavailable for first-generation electric/acoustic instruments.
Acoustic and bass pages expose their own named controls rather than JTV tuning.
Per-string tuning on p. 2•16 is expressly JTV-only; its half-step UI label is
not a recovered storage unit or full accepted numeric domain.

These family-level manual statements do not independently certify a particular
JTV-69/Variax 600 unit, firmware, editor format or bridge protocol. They do not
establish Workbench HD support through HD500X. The Workbench HD host-list gap
in [#61's evidence](line6-cross-product-evidence.md) remains distinct.
Page 2•11's description of global/preset Variax control contains an explicit
exception for tuning, which remains per-preset. Do not flatten that exception
into one global-state rule. No instrument control was exercised.

## Comparison and next evidence

The [existing driver record](pod-hd500x.md) identifies USB `0x0e41/0x4159`
and declares 48 kHz/24-bit defaults with format lock off. The manual's broader
rate options complement those defaults; they are not device negotiation proof.
The independently measured H5ES sample's 64 records are numerically consistent
with documented setlist capacity. That agreement does **not** authenticate its
product, header tuple, container layout or editor version. Existing ProX-label
ambiguity remains unresolved. Manual names cannot resolve file-suffix conflicts
between earlier Edit guides or register numeric model/effect/PARM identifiers.

Required next inputs remain an attributable original Edit package or defining
primary static resources, plus existing original-produced fixtures with known
product/version provenance. A future codec must test wrong-family/version
rejection, truncation/bounds, untouched unknown bytes and atomic save-copy;
these are a test plan, not tests run here. No prose-mirroring test is added.
Only source stat/hash, PDF identification and page/text inspection were performed
for this research. Required Rust checks are submitted separately through AC for
the actual published documentation head; their results do not validate manual
claims against hardware. All flags and locks remain unchanged. #32 stays open.
