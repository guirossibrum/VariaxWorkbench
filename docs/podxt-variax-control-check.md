# PODxt Live Variax control ownership and linked audition

**Not approved.** This is a design for a later owner check. It authorizes no
live run, no new message, and no tool. #232 stays open for its own items.
Nothing in this note is sent to the rig.

The check records who holds Variax control, and whether a linked model
auditions, on the owner's PODxt Live 3.01 and Variax 600 3.10. Every host
message it names was already approved and already observed. A later run, if
the owner approves one, repeats those occasions and fills in the EDIT-page
note or the dump that an earlier run left unrecorded.

Host bytes and the #227 observations are in
[the relay notes](podxt-variax-relay.md). Mismatch 6 in
[the capabilities audit](variax-300-600-capabilities.md) keeps standalone F-
recall separate from the USB release and the wrapped edit. The
[owner card](owner-tests/variax-capabilities.md) already asks for the last
Edit page, the USB state, the stored linked choice, and the guitar readback
as separate records.

## What each message changed

Labels are **confirmed** for a host encoding or an owner observation,
**likely** for a cause the ledger already marks unverified, and **unknown**
where the rig has not separated the question.

### `03 23 01`

```text
F0 00 01 0C 03 23 01 F7
```

**Confirmed** host meaning. Workbench mode 1 (`THRU_XTLIVE`) stores a
256-byte send chunk and then writes byte 6 as 1 on open. Monkey's
large-buffer enable is this same frame. The guitar bridge sends it before
guitar traffic. Workbench and Monkey wait for no SysEx reply. The 1,000 ms
sleep is Monkey's firmware writer, after a successful enable, and is not
part of the bridge open.

**Confirmed** on firmware 3.01. The PODxt accepted many sends of this frame.
No reset was observed. No device acknowledgement was recorded. What firmware
3.01 does with the byte is **unknown**.

**EDIT page.** This byte was not isolated as a display experiment. With
computer USB unplugged, the last EDIT page shows the Variax MODEL field.
Reconnecting USB reports computer control, with VARIAX set to ON. Those two
observations follow USB presence, which is the whole connection. The
internal ownership bit is **unknown**.

**Likely, unverified**, from the #201 owner run on 2026-11-24. No Variax
connect or disconnect notice reached the app. Plugging or unplugging the
guitar needed Refresh guitar. The ledger's likely cause is that the PODxt
forwards those notices only while this frame stays on, and the app turns it
off between exchanges. That cause was not isolated. Workbench's 11-byte
`03 11` notice and 12-byte `7F 03` notice are code-derived and are still
unobserved on this PODxt Live. This check adds no trial that leaves
`03 23 01` held on.

### `03 23 00`

```text
F0 00 01 0C 03 23 00 F7
```

**Confirmed** host meaning. Workbench mode 1 writes byte 6 as 0 on close.
Monkey's large-buffer disable is this same frame.
`SetXTLiveVariaxMessagesEnable(false)` is this frame. The Rust release
sender is this frame. There is no third `03 23` value in those templates.

**Confirmed**, owner run 2026-11-23. The capture shows one identity request
and one release on the PODxt MIDI port. The host dispatched the write. The
capture has no device acknowledgement. The last EDIT page continued to show
computer control. The MODEL field belongs to the USB-unplugged observation
above. Sending this frame while USB stays connected is a different action
from unplugging. The tone dump and the guitar metadata were not the recorded
subject of that release, so their change on that occasion is **unknown**.

A guitar metadata read ends by sending this same frame as bridge cleanup.
That cleanup is part of the read. It is a separate occasion from the
standalone release, whose subject was the EDIT page.

### Wrapped model command

```text
F0 00 01 0C 03 10 00 78 <value> F7
```

**Confirmed** encoding. Line 6 Edit `WrapVariaxMessage` for the electric
model field puts family opcode `0x10` under the PODxt header, then
controller 120 and the model value. Vetta's `03 10` is a different message
and is outside this check. The two values already sent are 15 (F-Spank 1)
and 10 (F-T-Model 1).

**Confirmed**, owner run 2026-11-23, on the tested USB route. The baseline
held linked value 10 and Telecaster on the guitar. The PODxt MIDI port
accepted value 15, the probe waited 10 seconds, the port accepted the
restore value 10, and the probe waited 10 seconds. All three PODxt dumps
matched byte for byte. All three guitar metadata pairs matched byte for
byte. The linked value stayed 10. The guitar stayed Telecaster. The EDIT
page for this wrap was not recorded, so the page result is **unknown**.
The wrapped audition had no observed effect on those dumps.

The positive model change on this rig is the owner's front-panel recall of
stored F- channels, with computer USB unplugged at each selection. That
recall is recorded below. It is not this wrapped command, and the check
adds no host program change. The approved 5B → 1B → 5B launcher stopped
before its first channel change.

## What the EDIT page shows

The page is the owner's visual note. It sits beside the two dumps. A page
and a dump are two results even when they agree.

| Occasion | USB | Last EDIT page | Label |
| --- | --- | --- | --- |
| Computer USB unplugged | unplugged | Variax MODEL field | **confirmed**, 2026-11-23 |
| USB reconnected | connected | computer control, VARIAX ON | **confirmed**, 2026-11-23 |
| One identity request and one `03 23 00` on the PODxt MIDI port | stayed connected | computer control | **confirmed**, 2026-11-23 |
| Wrapped values 15, then 10, on the tested USB route | connected | not recorded | **unknown** |

## Baseline

Use the wrapped-test starting state. The USB channel-recall run that stopped
on JazzBox-3 is a different baseline and stays out of this check.

| Record | Baseline |
| --- | --- |
| PODxt identity | PODxt Live, revision 3.01 |
| Variax identity | Variax 600, firmware 3.10 |
| PODxt tone | 168-byte edit buffer. The family-0 model at tone offset 152 is 10 (F-T-Model 1). That offset is byte 159 of the frame, the CC 120 slot. |
| Variax metadata | Name T-Model-1, body `0x03100000`. The owner ledger calls this guitar Telecaster. |
| EDIT page | The owner's note, taken with USB connected, before the subject message of that occasion. |

A later occasion starts only when the tone integer, the guitar name, and the
guitar body match this table. Any other current state stops that occasion.
The three PODxt dumps and the three guitar pairs from 2026-11-23 matched
this tone integer and this guitar throughout the wrapped pair.

Standalone F- recall, already recorded, is a result and not this baseline.
With USB unplugged, the owner selected channel 1B and returned to 5B. After
USB returned, the guitar metadata was Chime-1 (body `0x0310000c`), then
T-Model-1 (body `0x03100000`). The verified backup names those stored
channels 1B (F-Chime 1) and 5B (F-T-Model 1). No preset was saved.

## Independent readback

Each stage keeps three records. Each record is kept when the others agree.

1. The last EDIT page, and whether computer USB is connected.
2. The PODxt current tone. Identity on the PODxt MIDI port, then
   `F0 00 01 0C 03 75 F7`. The reply is the 168-byte frame that begins
   `F0 00 01 0C 03 74`. The family-0 linked integer is tone offset 152.
3. The Variax current metadata, through the existing bridge read: Variax
   identity, session `2A 01`, the two 32-word `0x3C` reads from address
   851, and session `2A 00`. Each reply is 200 bytes. The name and the body
   come from that pair.

The guitar read's bridge open is `03 23 01` and its bridge close is
`03 23 00`. Those two frames belong to the read. On the release occasion
the owner notes the EDIT page after the standalone `03 23 00` and before
this guitar read, because the read's open sends `03 23 01` again.

A host accept records dispatch. Restoration is the dump comparison below.
The internal ownership bit is not read by either dump.

## Restoration

Restoration is byte equality of the PODxt tone with its baseline dump, and
byte equality of both Variax metadata replies with their baseline replies,
together with the owner's page note for that occasion.

The wrapped occasion restores by sending value 10 again, waiting 10 seconds,
and reading both devices. The 2026-11-23 run met the dump half: the dumps
matched the baseline because the edit had left them unchanged.

The release occasion has no second message that returns a previous page. Its
dump comparison is the same equality, taken after the page note and the two
reads. The 2026-11-23 release left the page on computer control and did not
record the dumps, so that dump half is still unmeasured.

One matching dump leaves the other dump's comparison open. The page note is
kept either way.

## Keep-current, F-, U-, and Custom

The family-0 model list is Line 6 Edit `VARIAX_FAMILY_0_MODEL` (CC 120, tone
offset 152). The labels are a static fact from that list. A label is not an
observed device change. In the list, Custom 2 (56–60) sits between F-Reso 1
(55) and U-T-Model 5 (61).

| Choice | Family-0 values | On this rig |
| --- | --- | --- |
| Keep-current | 0, label "(Don't Control)". Handbook p. 4•11 is the setting that keeps the guitar's current model when the channel is recalled. | Not sent. Not observed. |
| F- | 6–55, from F-T-Model 5 through F-Reso 1. Value 10 is F-T-Model 1. Value 15 is F-Spank 1. Value 35 is the list's label for F-Chime 1. | Front-panel recall of stored 5B and 1B, USB unplugged at selection, followed by T-Model-1 and Chime-1 on the guitar. Wrapped 15, then 10, left both dumps unchanged. Every other F- value was not sent. |
| U- | 61–110, from U-T-Model 5 through U-Reso 1. | Not sent. Not observed. |
| Custom | 1–5, Custom 1 positions 5 down to 1. 56–60, Custom 2 positions 5 down to 1. | Not sent. Not observed as a PODxt linked choice. These values are separate from selecting a Custom bank on the guitar by hand. |

The other family model fields have their own controllers and were not sent.
Family 1 is CC 96 at tone offset 128, with acoustic F- and U- names and its
own value 0 "(Don't Control)". Family 2 is CC 122 at tone offset 154, with
its own value 0 and its U- names. Families 3 and 4 alias the family-0 list
and use CC 126 (offset 158) and CC 99 (offset 131). The family selector is
CC 101 at tone offset 133. The observed wrap is controller 120 only.

Coverage of the four choices inside this check is a classification of
integers already present:

- Tone offset 152 in the baseline `03 75` read.
- The verified backup's stored names for 5B (F-T-Model 1) and 1B (F-Chime 1).

When the current integer is 0, a Custom value, or a U- value, the check
records that integer, the EDIT page, and the guitar metadata, and sends no
model command. It does not change channel to go looking, and it does not
request a new stored-slot dump.

## Recording occasions

**Not approved.** The three occasions stay separate. Each one starts from
the baseline above and stops when the baseline does not match. The owner
runs a live pass only after approving that pass.

### Occasion A — `03 23 01`

The baseline guitar read is the `03 23 01` for this occasion. Record the
EDIT page after that bridge open, and keep the two metadata replies as the
guitar half of the baseline. The page already known for a USB reconnect is
computer control with VARIAX ON. The effect of the byte alone stays
**unknown**, as above.

### Occasion B — wrapped F- audition

The pair already sent, and only that pair:

1. `F0 00 01 0C 03 10 00 78 0F F7` (15, F-Spank 1). Wait 10 seconds. Note
   the EDIT page. Read the PODxt tone and the Variax metadata.
2. `F0 00 01 0C 03 10 00 78 0A F7` (10, F-T-Model 1). Wait 10 seconds. Note
   the EDIT page. Read both again.

On 2026-11-23 this route left both dumps unchanged and left the linked value
at 10. Restoration is dump equality with the baseline. The page note is the
part that run left unrecorded.

### Occasion C — `03 23 00`

One identity request and one `F0 00 01 0C 03 23 00 F7` on the PODxt MIDI
port. USB stays connected. Note the EDIT page before any guitar read. Then
read the PODxt tone and the Variax metadata. On 2026-11-23 the page stayed
on computer control. Dump restoration for this occasion is the criterion
above and is still unmeasured.

### Choices left as reads

Keep-current, U-, Custom, and every F- value other than 10 and 15 stay in
the classification table. A current tone that already holds one of them is
recorded by the baseline reads. Selecting it would be a new message.

## What stays unknown

- The internal ownership bit.
- A device byte in answer to `03 23`.
- What firmware 3.01 does with either `03 23` value.
- The EDIT page during the wrapped pair.
- Whether the PODxt can audition a linked model while USB is absent. The
  observed model change is the front-panel F- recall with USB unplugged at
  selection.
- Keep-current, U-, and Custom on this rig.
- Whether holding `03 23 01` is what lets Variax connect and disconnect
  notices through. The #201 cause stays unverified, and this check does not
  add that trial.
