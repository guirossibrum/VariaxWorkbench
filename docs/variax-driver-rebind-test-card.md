# PODxt Live guitar driver return

Owner-run check for [#239](https://github.com/guirossibrum/VariaxWorkbench/issues/239).
Review this card before running it. Synthetic sysfs and fake transports cover
bind results and USB address changes offline. Physical recovery awaits this run.

## Setup

- Use the supported Linux rig with PODxt Live firmware 3.01 and Variax 600
  firmware 3.10. Record the review commit and the kernel version.
- Build the app and, if you are running an already approved owner procedure
  separately, rebuild its tool from the same commit. Close other editors and
  old copies of the tools. Capture terminal stderr from this run in a new log.
- Use the existing Device check step 1 to inspect the binding. Leave Device
  check steps that send requests closed for this card. Leave the USB cable
  connected during the recovery check. Use a naturally occurring unbound
  interface for the retry case. Do not deliberately unbind or reset it.

## Driver return and recovery

1. With the rig connected, start the app from the terminal. Check that the
   Variax tab appears only after a valid guitar identity. Use **Refresh guitar**
   once. After the check, look for `PODxt Live interface 1: snd-usb-variax
   binding verified.` and verify the same binding in Device check step 1.
2. If the guitar driver is unavailable and the PODxt Live tab still has its
   verified identity, choose **Refresh guitar**. Check that the dialog defaults
   to **No**. Press Enter and verify the app stays on the unavailable status
   with no reattach or identity check.
3. Choose **Refresh guitar** again and select **Yes, reattach**. Expect a
   binding check, at most one reattach attempt, and a verified binding before
   the usual guitar identity check. Expect the existing Variax tab to
   reattach after a valid reply. If the driver still fails to bind, record the
   error. Replug the PODxt USB only as a last resort after the operation has
   stopped.
4. If the PODxt resets during normal use, note when it happened. Expect the
   previous attachment's cleanup to stop on a changed address and the app
   to check the new attachment normally. Record any recovery prompt that
   was open at the time and whether it acted on the old attachment.
5. During any separately approved reso5-test or variax-probe run, check the
   terminal log after interface 1 cleanup. Expect the same binding result.
   If the ordinary driver return leaves it unbound, expect one short wait,
   at most one retry, and another verification. Follow the tool's existing
   procedure and confirmations.

The app sends no guitar commands during reattachment. Linux may send its
own startup traffic as the driver binds. After the binding is verified, the
app's existing guitar identity procedure applies. Record whether you exercised
the naturally unbound case or only verified the normal return.
