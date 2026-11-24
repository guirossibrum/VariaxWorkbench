# Variax library backup owner test

[#207](https://github.com/guirossibrum/VariaxWorkbench/issues/207) · Variax 600
through PODxt Live · Device check G5

1. Note the current guitar bank and selector position. Open Device check,
   choose **Back up the Variax**, and confirm.
2. Expect **60 of 60 presets** and **12 of 12 bank names**. Check the switch-back
   result against the starting preset. Stop and record the message and backup
   folder if the sweep stops or cleanup fails.
3. Compare the new backup folder with the reference. Set `new_backup` to the
   folder reported by Device check and run this from the repository:

   ```sh
   cargo run --offline --manifest-path tools/variax-probe/Cargo.toml -- compare \
     "$HOME/Downloads/variax-library-20261124T025803Z" "$new_backup"
   (cd "$new_backup" && sha256sum -c MANIFEST.sha256)
   ```

   Expect **120 identical preset files**, no differences, and a clean manifest,
   including all 12 files under `bank-names/`.
4. Open the new `index.txt`. Check the 12 `Bank … name` entries in model-knob
   order (00–11) against the names you expect on this guitar, including custom
   banks. Report any discrepancy with the bank number and displayed name.

## Full program backup (#236)

The first library-wide run is pending owner and reviewer confirmation. The
single-slot #234 run verified the 35-chunk read on this rig. Only the owner
runs this card. Keep both existing settings backups and any XP-era copies.

1. Use the PODxt Live 3.01 and Variax 600 3.10. Wait for the Variax tab to
   show a verified identity. Note the starting guitar bank and selector
   position. Keep the guitar controls and PODxt channel steady.
2. In Device check choose **Back up all Variax program bytes**, confirm,
   and select a destination. Allow several minutes for 2,100 chunk requests
   plus metadata reads and bank names. A timed-out chunk can take up to
   10 seconds. The original **Back up the Variax** remains settings-only.
3. Expect **60 of 60** verified programs and **12 of 12** bank names.
   Check the switch-back message and confirm that the guitar responds
   normally. **Stop full backup** waits for the current request and then
   attempts switch-back and driver return. An interrupted or failed run
   leaves an `-INCOMPLETE` folder and identifies missing programs in
   `index.txt`. If a chunk stalls and the controls freeze, follow the
   recovery notes in the [#234 test card](reso5-full-read-test-card.md).
4. With `new_backup` set to the reported folder, check the manifest and
   compare flash slot 50 (Reso position 5, MIDI program 105) with the
   expected capture.

   ```sh
   (cd "$new_backup" && sha256sum -c MANIFEST.sha256)
   sha256sum "$new_backup/programs/50-Reso-5.bin"
   ```

   The [#234 owner capture](https://github.com/guirossibrum/VariaxWorkbench/issues/234)
   was `3d8bae813991fa5996fb649ad80d116de8ef358c03724770183b89cdef5ad249`.
   Use `90d79131f7909639aa2d0793cb3f617fbd95a95a1ada7a54f90ec84de4e28164`
   if the [#177 factory store](reso5-store-test-card.md) has since succeeded.
   If another approved store mode ran, use that mode's recorded hash.
   Record the observed hash and any discrepancy. A full backup preserves
   the program bytes returned after recall. The firmware checksum checks
   integrity but cannot prove slot identity if a recall silently fails.

Each verified file under `programs/` is 3,276 bytes. Keep the manifest and
index alongside the program files. No file is sent back to the guitar.
