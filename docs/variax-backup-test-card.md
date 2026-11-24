# Variax library backup owner test

[#207](https://github.com/guirossibrum/VariaxWorkbench/issues/207) · Variax 600
through PODxt Live · Device check G5

1. Note the current guitar bank and selector position. In Device check,
   select **This computer**, choose **Back up the Variax**, and confirm.
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
