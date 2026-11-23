# Variax 600 and PODxt family schematic reference

The Variax 600 service PDF covers the six-string signal path, processor,
memory, physical controls, and guitar-side digital connection. The POD PDF
covers the standard PODxt SHARCPOD board. It is a hardware reference for the
family, with PODxt Live board details still to be established independently.
Both sources help constrain offline firmware analysis and the review of paths
that could change persistent state.

## Local PDFs for double-checking

The original scanned PDFs are available in the owner's Downloads folder on
the source machine. Use the filename and SHA-256 to locate and verify the
copy. The PDF page numbers below are one-based, and several pages open
sideways. The PDFs remain local vendor references outside Git.

| PDF | Pages | SHA-256 |
| --- | ---: | --- |
| `Variax600_schematics.pdf` | 5 | `d34a63fafd6232f87b72ec393564f506c5c4f7a69eb5bb50d3cce01a609e8a17` |
| `PodXTLive_schematics.pdf` | 7 | `d5d9ca538874daac4ffd2484ae14f31fa8f32c9e2b4dbec56b4a7407e35b0d6a` |

## Variax 600

The packet includes a V600-specific bridge sheet and MG-300 electronics
sheets. The designators identify the circuits shown in those drawings.
The installed board and component markings on the owner's guitar remain
unverified.

| PDF page | Drawing | Circuit in words |
| --- | --- | --- |
| 1 | MG-300 MAIN, Analog + Supply. Rev A1, drawing 35-00-0047, sheet 1/2 | Six piezo signals enter the analog front end separately. The high E/B pair goes to U4. The G/D and A/low E pairs go to the AK5384 at U5. The same sheet shows the DAC output buffer and supply paths from the battery, TRS jack, and RJ45 connection. |
| 2 | MG-300 MAIN, DSP Memory UI. Rev A1, drawing 35-00-0047, sheet 2/2 | DSP56367 at U11 connects to SST39LF040 flash at U15 and IS61LV6424 SRAM at U14. The sheet shows reset, boot-mode connections, flash select and read/write signals, and a JTAG header marked unpopulated for production. Volume, tone, model, Save, and five-way inputs go to a separate UI A/D circuit. The digital harness carries separate AES audio and MIDI control signals. |
| 3 | V600 G1-1 Tremelo Bridge. Rev A, drawing 35-00-0068 | The bridge harness carries one piezo connection per string. Use this sheet to check string identity and ordering against the firmware's channel mapping. |
| 4 | MG-300 Output. Rev A, drawing 35-00-0063 | The quarter-inch jack carries the analog guitar output, an external-power connection, and jack-switch signals. |
| 5 | MG-300 U/I PCB. Rev B, drawing 35-00-0048 | The control board includes the physical tone and volume pots, model selector, Save switch, and five-way selector. The harness connects the AES, MIDI, and power signals toward the digital jack. |

The signal path runs from the six bridge piezos through the analog input and
conversion stages to the DSP. The DSP has separate paths for the analog
output DAC and the AES audio signal. The controls have their own conversion
path. This distinction matters when comparing hardware measurements with
virtual pickup or pot parameters in a preset. The processor marking on page 2
corroborates the DSP56300-family interpretation of the archived 3.10 image.
[Firmware map](firmware-map.md#variax-300600-archive-310)

## PODxt family reference

Despite its filename, the parts list identifies **POD XT US** and the circuit
sheets identify **SHARCPOD MAIN**. The PDF contains five excerpts from
revision C1 drawing 35-00-0134, followed by two parts-list pages. Use the
Live owner's manual for the pedalboard's Bank Up/Down, A–D, and block
switch workflow.

| PDF page | Drawing | Circuit in words |
| --- | --- | --- |
| 1 | DSP, sheet 5/9 | The ADSP-21065L at U13 connects to the guitar ADC, direct-output DAC, USB audio data lines, and the controller's handshaking signals. Address, data, reset, and boot-mode connections give reference points for studying the SHARC loader. |
| 2 | DSP Memory, sheet 6/9 | Lower and upper SDRAM positions support alternate 1M × 16 or 4M × 16 devices. The flash at U14 is marked AM29LV800BB, with AM29LV160B listed as a substitute. The drawing records options for this board. The populated memory parts and addresses on a Live board remain unverified. |
| 3 | Interface, sheet 7/9 | An RJ45 connects the FBX foot controller interface. Separate circuitry handles MIDI IN and OUT. This RJ45 is identified as FBX on the drawing. The Live VDI connector needs its own model-specific reference. |
| 4–5 | Pots / LEDs and Switches / Encoders / LCD, sheets 8–9/9 | Analog panel pots feed an A/D converter. Switches and encoders feed a scan circuit, and separate circuits drive indicators and the display. These are the standard PODxt panel controls. |
| 6–7 | Parts lists | Page 6 identifies the POD XT US assembly and the SHARCPOD main PCBA. Page 7 continues the main-board parts list. Use these identifiers to check product scope before reusing a circuit assumption. |

The SHARC drawing is consistent with the architecture inferred from the
archived PODxt Live 3.01 firmware. A match at the processor-family level
leaves the Live board's exact chip, flash geometry, reset behavior, and
VDI-to-USB routing open. [Firmware map](firmware-map.md#podxt-live-archive-301)

## Use in the hardware gate review

Processor and memory evidence helps distinguish working RAM from persistent
storage and locate the boot, reset, and flash-programming paths worth tracing.
Firmware file offsets, DSP word addresses, and physical flash addresses each
need their own evidence. The schematics establish how the illustrated circuits
are wired. Protocol captures and a firmware dispatch trace establish what an
identity request does.

For G2, the reviewed request is one universal identity inquiry, with the
Variax 600 reply as the expected answer. A static search of the decompressed
Variax 3.10 and PODxt Live 3.01 archives did not find that request or its
reply header, so no path from the request to reset, bootloader, erase, or
program code was established. The Variax sheet shows the DSP56367 and
SST39LF040; the POD sheets are the standard SHARCPOD board, not a confirmed
Live board. Interface-1 detach, release, and reattach remain Linux driver
operations and were not observed in the captures. The offline simulation
sends one request and is not a device result. The normal app opens G2 for
any user of the same supported rig; this schematic review does not run it.
[G2 ledger](hardware-gates/g2.md)

For later backup or recovery work, correlate any candidate flash map with
the exact board revision, populated flash part, and firmware image. Current
knowledge supports static review. A verified backup and a reviewed restore
procedure remain separate deliverables. [Hardware gates](hardware-gates.md) ·
[Update and recovery evidence](update-recovery.md)
