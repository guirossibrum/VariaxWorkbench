# PODxt Live macOS driver

Evidence labels distinguish **Static fact**, **Inference** and **Unknown**. Addresses refer to the named Mach-O slice. Scope is the locally extracted Line 6 Audio-Midi Driver 7.6.8 package.

## Inventory

| Binary | Static fact — architecture, version and external interface | Inference — loader |
| --- | --- | --- |
| `Line6MIDI.plugin/Contents/MacOS/Line6MIDI` | Universal x86_64 and i386 bundle, both plist versions 7.6.8. Factory and only global defined text symbol `_NewL6MIDIDevManager` at `0x1a70` (x86_64) and `0x1b90` (i386) | CoreMIDI server via CFPlugIn registration |
| `Line6AudioSigned.kext/Contents/MacOS/Line6Audio` | x86_64 only, both plist versions 7.6.8. Kernel symbols include `com_line6_audio` class, `com_line6_driver_isoenginext` and local `__start` at `0x9c75f` | IOKit kernel extension matching registered USB personalities |
| `L6TWX_u.dylib` | i386 only. Embedded install ID under `L6TWXY.framework/Versions/A/data/twx/`, compatibility/current version 1.0.0. Exports the `TWX*` interface listed below | A Line 6 application or framework client, exact consumer in this distribution unknown |

**Static fact.** The MIDI plug-in links Carbon, IOKit, CoreMIDI, CoreFoundation, libstdc++ and libSystem. Its i386 slice also links CoreMIDIServer. Undefined symbols include `IOServiceMatching`, `IOConnectCallMethod`, `IOConnectCallAsyncMethod` and `MIDIDeviceCreate`. Its plist gives the CFPlugIn factory and type UUIDs, without a USB VID/PID personality.

**Static fact.** The kext's `OSBundleLibraries` declares AppleUSBMergeNub, IOAudioFamily, IOUSBFamily, and the iokit, libkern and mach KPIs. Kernel undefined symbols include `IOUSBDevice` and `IOUSBInterface` methods. It exposes kernel C++ class symbols and associated vtables. Under vendor `0x0e41`, PODxt `0x5044`, PODxt Pro `0x5050` and PODxt Live `0x4650` each have a `com_line6_audio` personality for `IOUSBInterface`, configuration 1, interface 0. Each also has two `IOUSBDevice` personalities, `AppleUSBMergeNub` and `AppleUSBComposite`. The interface-0 personality agrees with the POD side of the [Linux split](linux-transport.md). The device-level personalities alone give no evidence about interface-1 transfers.

**Static fact.** `L6TWX_u.dylib` links SystemConfiguration, Carbon, CoreAudio, CoreFoundation, CoreMIDI, CoreServices, IOKit, ApplicationServices, libcrypto, libstdc++, libgcc and libSystem. The 15 global text names are `TWXBeginDecrypt`, `TWXCommandInterface`, `TWXDecrypt`, `TWXEndDecrypt`, `TWXExit`, `TWXGetHardwareDeviceInfo`, `TWXGetLastErrorA`, `TWXGetLastErrorW`, `TWXGetNumHardwareDevices`, `TWXGetVersionA`, `TWXGetVersionW`, `TWXInit`, `TWXIsValidAsset`, `TWXLaunchMonkey` and `TWXVersionToString`. It has no separate USB matching plist in the examined payload and is absent from the MIDI plug-in's dependency load commands. The [prior USB firmware analysis](pod-usb-firmware.md) records Line 6 device-manager classes inside the dylib.

**Inference.** These Intel binaries provide no native Apple Silicon driver. An Intel slice in the user-space MIDI bundle does not supply an ARM64 kernel driver. **Unknown.** The host versions that will accept the legacy Intel package and the exact process that loads the TWX component.

**Static fact.** Inventory can be reproduced with `file`, Python `plistlib` and `llvm-objdump --macho --arch=<slice> --dylibs-used --syms`.
