# Owner test card for the Variax 300/600 capability audit

These checks describe evidence still needed after #227. The owner runs any
physical test. Each action that changes saved data or control state requires
its approved procedure. Record the rig, firmware, connection route, selected
positions and observations. Keep recordings and device dumps in ignored local
storage.

| Check | Procedure and result to record |
| --- | --- |
| Physical numbering and controls | On the guitar, sweep positions from bridge to neck in an unchanged bank. Read the displayed/preset identity independently at each position with an already approved settings-read procedure. Note the physical end and matching backup program number. Repeat in a Custom bank without using the hold-to-save gesture. |
| Model-specific audio | With matching factory models and a consistent listening setup, compare electric volume roll-off and acoustic level. Compare T-Model 4 and 5 and Spank 1 at both tone extremes. Compare the changes on an acoustic model and Reso 2. Record whether the differences are consistent with bypass, filtering, mic movement or drone level. Use only separately authorized capture, if any. |
| Saved versus live tone recall | Select a source model and a sacrificial Custom position with a verified backup and an owner-approved restore. In the same position, compare the handbook's default save and the save with the tone knob moved during the hold. Before recall, set the physical knob differently. Capture the saved mode and position with the approved read, observe the initial sound and the first knob movement, and verify restored settings after the approved restore procedure. Stop if the baseline, target, restore or readback is uncertain. |
| Standalone and USB-linked POD recall | Use the existing 5B → 1B → 5B evidence as the F- baseline. For any further approved linked procedure, record the last Edit page, USB state, stored linked choice and guitar readback for each transition. A stable initial match is required before a channel or model change. Evaluate U-, Custom and keep-current only with separately approved pre-existing tones. |
| USB ownership handoff and audition | Record the display and actual guitar response independently. Compare before and after the separately approved release or temporary edit, using the original procedure for each. A successful host send and a display state alone do not establish a change. Verify the initial tone and guitar settings again at the end. |
| Preset payload and bank names | For slot 50/current/spare, use the #225 card and its exact scope. For any separate owner-approved bank-name read, record received name bytes alongside physical bank and current preset. Keep the results distinct from the two-block metadata backup. |
| Store and restore | Use the dedicated [Reso 5 store card](../reso5-store-test-card.md) and the separately approved #178 procedure. A passing full edit-buffer check provides no store or restore result. |

Service, firmware install and reset remain outside this card.
