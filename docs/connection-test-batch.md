# PODxt Live connection test batch

The owner runs this [#201](https://github.com/guirossibrum/VariaxWorkbench/issues/201)
test on their own rig. The script timestamps connection changes and can
record USB traffic through an existing TShark USB capture interface. It does
not open MIDI ports or send device commands. The OS drivers and any editor
you open can send their usual commands. Original Workbench 1.75 can resend
its current model on reconnect. Run the original-editor scenario only under
the approved owner procedure.

Use Python 3.9 or later. On macOS, run from the repository in Terminal:

```sh
python3 scripts/connection-test-batch.py --markers-only
```

On Windows, run from the repository in PowerShell:

```powershell
py -3 .\scripts\connection-test-batch.py --markers-only
```

The default comparison guides three VDI disconnect/reconnect cycles with
both editors closed, then three with **Line 6 Workbench 1.75** open. Use
`--scenario workbench` for a separate batch with **Variax Workbench**, or
`--scenario closed` for the first batch alone. `--attempts 5` changes the
cycle count. Leave the PODxt plugged in to USB throughout each set of cable
changes. Press Enter just after each physical action and record what you
saw. The Windows build of Variax Workbench currently has no live connection
backend. Use the original editor on a Windows machine where it already
runs if you want the original comparison.

## Add USB evidence

Run the script with `--list-usb` to list the USB interfaces TShark exposes.
Replace `--markers-only` with `--usb-interface N`, where `N` is the entry
for the PODxt's USB controller. Windows requires an already available
USBPcap interface. macOS needs a USB capture interface visible to TShark.
If none is available, `--markers-only` still produces comparable timing
and observations. The script changes no drivers or OS security settings.
[Wireshark USB capture documentation](https://wiki.wireshark.org/CaptureSetup/USB)

Each packet capture defaults to 120 seconds. Use `--seconds 180` to allow
more time. Start the capture before attaching or powering up the PODxt
if the approved test calls for recording its startup. In the original
editor batch, wait for the script to tell you when to open the editor.
Use the same cable and timing for both batches.

The script creates a fresh output folder under `Variax connection tests` in
your home directory, with an `events.csv` per scenario. If capture is
enabled, it also saves `usb.pcapng` and `capture.log`. Keep captures local.
A USB controller trace can include packets from other devices on the same
controller.

For packet review, find the PODxt Live's USB address and verify coverage of
both interrupt inputs `0x84` and `0x86`. Look at the outgoing transfers before
and during each run, especially on `0x03` and `0x05`. Match the capture to the
UTC cable-change markers. An editor-closed run can still contain startup
and identity messages from the driver. A populated capture file alone does
not establish that the PODxt or both input endpoints were observed.
