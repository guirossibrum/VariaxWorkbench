# Initial Workbench 1.75 investigation

## Scope and status

Target is a Variax 600 through PODxt Live on Linux. Installer extraction and Java
decompilation are complete. Device communication and native DSP conversion are
unverified. No installer or original application UI was run and no hardware messages
were sent. The portable JDK and CFR were run to inspect the application. A small
Java harness also called the original static word encoder without opening devices.

## Evidence

`analysis/reports/inventory.json` records installer hashes, sizes, native library
paths, and the cross-platform class comparison. Original installers are in
`vendor/installers/`; generated proprietary artifacts stay out of version control.

The Windows installer is NSIS. Its installed `Variax Workbench.exe` contains a ZIP
of Java classes and native libraries. The Mac DMG contains a XAR installer whose
gzip/CPIO payload contains `VariaxWorkbench.app`. Its application code lives in
`Contents/Resources/Java/VariaxWorkbench.jar`.

The Mac JAR has 661 classes. All 661 exist in the extracted Windows application;
659 are byte-identical. Only `L6VariaxWorkbenchApp` and `VersionInfo` differ.
This establishes shared Java logic, not cross-platform native compatibility.

CFR 0.152 output is in `analysis/decompiled/mac/`; its log is
`analysis/reports/cfr-mac.log`. This is decompiler output, not original source;
some methods carry control-flow warnings and require bytecode verification before
porting. Sources below are relative to that output directory.

| Finding | Static source |
| --- | --- |
| PODxt Live connection mode, MIDI enable, 256-byte SysEx chunks | `modules/variax/device/L6VariaxDevice.java`, `OnInit`, `EnableVariaxMIDI`, `SetXTLiveVariaxMessagesEnable` |
| Variax 300/600 minimum firmware check is 3.07 | same class, `MINIMUM_300_600_FIRMWARE_VERSION` and `CheckConnectionStatus` |
| Read current preset metadata in two blocks | `modules/variax/device/L6VariaxPatchManager.java`, `RequestDevicePatch` |
| Metadata decoding and parameter mappings | `modules/variax/device/L6VariaxMIDITranscoder.java`, `DecodePatch` |
| Native MIDI port enumeration, open, receive and transmit | `cmn/l6java/midi/L6MIDIJava/L6MIDIJava.java` |
| Native model-to-DSP conversion and body/pickup loading | `modules/variax/device/coefeng/L6VariaxCoefEngine.java` |
| SysEx framing | `cmn/l6java/midi/L6MIDIManager.java`, `_SendSysex` |

## Recovered messages

Channel zero messages, derived statically; not tested on hardware:

* PODxt Live Variax routing enable: `F0 00 01 0C 03 23 01 F7`.
* Variax SysEx header: `F0 00 01 0C 07 00`; terminator: `F7`.
* Enable Variax MIDI: header + `2A 01` + terminator; original waits 100 ms.
* Read metadata: header + `3C` + count word + address word + terminator.
  Words are six nibbles, least significant first. Count is 32; addresses are 851
  and 883. Original expects 200-byte replies with opcode `3C` at offset 6 and
  copies 192 payload bytes from offset 7, waiting up to 2500 ms per reply.
* The 384 combined nibble bytes decode to 192 metadata bytes. Each group of six
  nibbles decodes to three bytes in reversed pair order. First 18 decoded bytes
  are the patch name; metadata follows. This is not a full DSP patch backup.

`scripts/protocol_preview.py` prints these messages without any transport.
Requesting another preset in the original code sends a program change first;
even retrieval therefore needs session-state care. Original block wrappers may
also bypass DSP temporarily. A future implementation must handle cleanup and
restore the prior session state.

## Linux obstacles

1. Java MIDI uses a platform-specific JNI library, not just `javax.sound.midi`.
   Its loader chooses Windows or Mac; a Linux backend must replace that path.
2. `nativeCreateHWPatch` generates a 3276-byte DSP patch. Body/pickup editing
   depends on this native coefficient engine; translating the Java UI alone is
   insufficient. Windows includes a 32-bit x86 DLL; Mac includes a universal
   PowerPC/i386 Mach-O library. Neither is a native Linux library.
3. The initial pass could not run REA with the system Node 26.8.2. This is now
   resolved: REA 6.9.0 is built with local Node 24.22.1/npm 11.16.0 and runs with
   local Ghidra 12.2/JDK 25.0.5. See `native-analysis.md` for subsequent evidence.
4. Both the initial bounded sysfs/ALSA checks and a subsequent unsandboxed `lsusb`
   scan show no Line 6 device. No MIDI device nodes are present. Hardware testing
   awaits the powered PODxt Live's USB connection.

The Mac coefficient library retains C++ symbol names, including
`L6VariaxHWPatch::SetBodyResonatorCoefs` and pickup-response setters, plus its JNI
entry points. The Windows DLL exports the six expected JNI functions. These are
useful native-analysis entry points; symbol presence alone does not recover the
coefficient algorithms. Raw static output is saved in
`analysis/reports/coefengine-mac-strings.txt` and `coefengine-pe.txt`.

## Offline validation completed

`scripts/VerifyWordEncoding.java` calls `L6VariaxDevice.Int2VXWord` directly from
the original Mac JAR for 0, 32, 851, 883, 0x123456 and 0xffffff. The Python encoder
matches all six outputs, saved in `analysis/reports/original-java-word-encoding.txt`.
The method's original bytecode is retained in `analysis/reports/L6VariaxDevice.javap.txt`.
Both generated metadata requests are 20 bytes. Neither this comparison nor the
decompilation verifies POD routing or complete request/response behavior on hardware.

## Reproduction

Tools acquired directly from official sources: portable 7-Zip 26.06
(`https://github.com/ip7z/7zip/releases/download/26.06/7z2606-linux-x64.tar.xz`),
Temurin JDK 17 via Adoptium's Linux x64 JDK endpoint, and CFR 0.152
(`https://www.benf.org/other/cfr/cfr-0.152.jar`). They live under `.tools/`.
JDK and 7-Zip upstream downloads were extracted locally, not installed system-wide.

Extraction sequence, from project root:

```sh
.tools/7zip/7zz x -y -oanalysis/extracted/workbench-windows 'vendor/installers/Variax Workbench v1.75 Installer.exe'
.tools/7zip/7zz x -y -oanalysis/extracted/windows-app 'analysis/extracted/workbench-windows/Variax Workbench/Variax Workbench.exe'
.tools/7zip/7zz x -y -oanalysis/extracted/workbench-mac 'vendor/installers/Variax Workbench 1.75.dmg'
.tools/7zip/7zz x -y -oanalysis/extracted/mac-package 'analysis/extracted/workbench-mac/Variax Workbench/Variax Workbench.pkg'
mkdir -p analysis/extracted/mac-app
bsdtar -xf analysis/extracted/mac-package/Variax_Workbench.pkg/Payload -C analysis/extracted/mac-app
.tools/jdk/bin/java -jar .tools/cfr-0.152.jar analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar --outputdir analysis/decompiled/mac --silent true
python scripts/inventory.py > analysis/reports/inventory.json
python scripts/protocol_preview.py
```

## Next milestones

1. Word encoding is verified; verify complete request construction and parsing
   against captured replies next.
2. Identify the PODxt Live's available Linux USB/MIDI endpoints with it connected;
   implement identity detection and current-preset reads before preset switching.
3. Inspect the native coefficient engine and separate features that can work
   without regenerating DSP data from those that require it.
4. Establish a complete backup/restore format and validated transport before
   implementing persistent preset writes or firmware operations. Monkey has since
   been extracted and its native resources and configuration inspected; see the
   subsequent native-analysis report.
