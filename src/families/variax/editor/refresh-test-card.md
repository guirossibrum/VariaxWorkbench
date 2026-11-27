# Variax Refresh owner test card

Use the verified Variax 600 3.10 device tab through the supported PODxt Live.
Record the selected guitar model before starting. Use a preset whose saved
settings uniquely identify it in the library so the read can verify the
switch back.

1. Open a file in that tab, make an unsaved edit, then choose **Refresh from
   guitar…**. Choose **Keep editing** and verify the original edit remains.
2. Choose **Download from guitar…** again, choose **Download and replace**, and
   watch the progress count. The existing file view remains in place until all
   60 presets and 12 bank names are available.
3. Check all 12 bank names and five presets per bank against your backup.
   Verify the guitar returned to its original preset. Record any switch-back
   or bridge-release warning and stop the check if one appears.
4. Use **Save As…** with a new .vwb filename. Reopen the copy and compare the
   bank names and model settings. The original open file should remain intact.
5. Start another refresh and choose **Cancel read** partway through. Verify the
   complete open library stays in the editor. Record the reported switch-back
   result. Check the physical guitar's final model.

The editor uses the G5 sweep and preset selection, G3 settings reads and the
G7 bank-name read policy for `0x6D`. DSP stays active. Refresh switches presets
temporarily and attempts a verified switch back when it finds a unique match
among the presets it read. It sends no preset edits or store commands.

## macOS explicit download owner acceptance (#310)

Use the verified private macOS USB build of **Variax Workbench** containing
`4ea77b8425ba02e3ea140667fd2f75e6ecea7b91`. Record the actual build number,
source SHA, package profile, macOS version and Mac architecture. Use one
PODxt Live 3.01 over USB with a Variax 600 3.10 on VDI. Keep Rig Check and
other applications using the rig closed. Note the USB route (direct or hub)
and the starting preset's bank, selector position and displayed name.
Choose a saved preset whose settings are unique in the library, preferably
early in the sweep if testing cancellation. Have a current settings backup
for comparison. Keep the guitar controls and PODxt channel steady while the
sweep runs. [Private USB build](../../../../docs/release-build.md#private-owner-usb-build) ·
[Settings backup card](../../../../docs/variax-backup-test-card.md)

1. **Check USB readiness and guitar identity.** A visible PODxt Live USB
   entry shows presence. Readiness requires a unique current USB attachment
   with an address and both MIDI interfaces published on the same macOS
   registry entry. Wait for the PODxt Live and Variax 600 identities to
   verify, then select the connected Variax tab. An interface-open error, a
   silent guitar after the interface opens, and an unverified firmware
   reply are distinct outcomes. If the PODxt is verified but the guitar is
   missing, use **Refresh guitar** if offered and record the result.
   [macOS USB readiness](../../../../docs/design/macos-podxt-usb-readiness.md)
2. **Authorize the sweep explicitly.** Verify that connection and guitar
   detection leave the library idle. Open a disposable file in the connected
   guitar tab and make a visible unsaved edit. Choose **Download from
   guitar…**. In **Replace unsaved changes?**, choose **Keep editing** and
   confirm the sweep stays idle. Choose **Download from guitar…** again,
   then choose **Download and keep current document**. With a clean
   document, the Download button starts the read directly. Record any
   macOS accessory permission prompt or refusal, any USB open or claim
   error, and any disabled-build message separately from the owner's
   choice to download.
3. **Wait for completion.** The footer counts presets up to `60/60`.
   Wait for **Read 60 guitar presets and 12 bank names** in the final status.
   That result requires all 12 bank-name reads, a successful read-back of
   the uniquely matched starting preset after reselection, and successful
   cleanup of the library bridge and macOS USB interface 1. Confirm the
   guitar is back on the starting model and responds normally after the
   operation. Record the observed return and any release error separately.
   This settings read switches presets temporarily and sends no preset
   edits or store commands.
4. **Check the open document and the download.** Confirm that the file's
   name, selection, unsaved edit and undo history survived the read. Expect
   **The download is ready to open.** Choose **Open downloaded library…**
   and use **Save As…** in the save prompt to preserve your edits in a
   separate file before opening it. An empty or untouched startup view
   can open a successful download directly. Check 12 bank names in order
   and five presets in each bank against the current backup. The
   downloaded library is unsaved. To keep it, use **Save As…** with a fresh
   `.vwb` filename, reopen the copy and confirm the original on-disk file
   is intact.
5. **Check cancellation.** Once a complete download is available, start a
   second read with an open edited file and choose **Cancel read** during
   progress. Allow the active request and cleanup to finish. The open file
   and its edits stay in place and the last complete download stays
   available. A failed read, unverified return, release failure or stale
   attachment must also withhold the new library. Stop on a return or
   release warning and record the guitar's actual final model. Early
   cancellation may leave too few read presets to identify a unique match
   and verify return.

If a stage fails, capture the exact footer status and error text or a
screenshot before clearing them. Include the build and rig details above,
whether USB and both identities verified, which choice you made, the last
progress count, the last bank or preset if known, and the time of any
disconnect or cancellation. Record any open/claim refusal or busy message,
missing bank name, switch-back or read-back result, and USB release warning.
Note whether the open file, unsaved edit and last complete download were
still available. Keep screenshots and any existing logs or test files
private under the ignored `backups/` area.

Track the automatic Home/Device connections startup window separately.
Windows download still needs a verified native transport and a live-capable
package. [#310 roadmap](../../../../docs/roadmap.md#pending-implementation-or-validation)
