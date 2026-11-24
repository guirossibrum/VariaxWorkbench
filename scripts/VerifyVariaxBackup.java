// Runs the original Workbench bundle read with synthetic replies and the
// recording MIDI boundary from VerifyVariaxWrite. No device is opened.
import cmn.l6java.core.L6ProgressSignal;
import cmn.l6java.device.L6Device;
import cmn.l6java.midi.L6MIDIManager;
import cmn.l6java.patch.L6Bundle;
import cmn.l6java.patch.L6PatchManager;
import cmn.l6java.utils.L6StaticUtils;
import modules.variax.device.L6VariaxDevice;
import modules.variax.device.L6VariaxMIDITranscoder;
import modules.variax.device.L6VariaxParamMap;
import modules.variax.device.L6VariaxPatchManager;
import java.util.ArrayList;

public class VerifyVariaxBackup extends VerifyVariaxWrite {
    static class BundleMidi extends Manager {
        int waits;
        public byte[] WaitForMIDIData(int timeout) {
            boolean bankName = waits++ % 11 == 10;
            if (timeout != (bankName ? 1000 : 2500)) {
                throw new AssertionError("Unexpected reply timeout " + timeout);
            }
            if (bankName) return BANK_NAME.clone();
            byte[] reply = new byte[200];
            System.arraycopy(HEX.parseHex("f000010c07043c"), 0, reply, 0, 7);
            reply[199] = (byte) 0xf7;
            return reply;
        }
    }

    static class BundleDevice extends Device {
        public boolean IsConnected(boolean update) { return true; }
    }

    public static void main(String[] args) throws Exception {
        RecordedMidi recording = allocate(RecordedMidi.class);
        recording.records = new ArrayList<>();
        L6StaticUtils.recordSleep = millis -> recording.records.add("pause " + millis);
        BundleMidi midi = allocate(BundleMidi.class);
        set(L6MIDIManager.class, midi, "m_midijava", recording);
        set(L6MIDIManager.class, midi, "m_F7", HEX.parseHex("f7"));

        BundleDevice device = allocate(BundleDevice.class);
        set(L6Device.class, device, "m_midi_manager", midi);
        L6Device.BankTopology[] banks = new L6Device.BankTopology[12];
        for (int bank = 0; bank < banks.length; bank++) {
            banks[bank] = device.new BankTopology("Bank " + bank, 5, 1, 0);
        }
        set(L6VariaxDevice.class, device, "m_topology", device.new DeviceTopology(banks));
        set(L6VariaxDevice.class, device, "m_param_map", new L6VariaxParamMap());
        set(L6VariaxDevice.class, device, "m_midi_transcoder", allocate(L6VariaxMIDITranscoder.class));

        L6VariaxPatchManager patches = allocate(L6VariaxPatchManager.class);
        set(L6PatchManager.class, patches, "m_device", device);
        set(L6VariaxDevice.class, device, "m_patch_manager", patches);
        set(L6VariaxPatchManager.class, patches, "EB_REQ_MSG", HEX.parseHex("3c0002000000000a0300000000"));
        set(L6VariaxPatchManager.class, patches, "GET_CURRENT_BANK_NAME_MSG", HEX.parseHex("6d"));

        device.OnInit();
        L6Bundle bundle = patches.RequestBundle(new L6ProgressSignal(60));
        // The app releases the bridge without calling OnClose's DSP-on send.
        call(L6VariaxDevice.class, device, "SetXTLiveVariaxMessagesEnable",
             new Class<?>[]{boolean.class}, false);
        if (midi.waits != 132) throw new AssertionError("Expected 132 replies");
        for (int bank = 0; bank < 12; bank++) {
            if (!"Custom bank".equals(bundle.GetBank(bank).GetName())) {
                throw new AssertionError("Unexpected bank name");
            }
        }

        // Reassemble Java's header/payload/terminator writes for byte
        // comparison at the MIDI-message boundary.
        StringBuilder sysex = new StringBuilder();
        int pauses = 0;
        for (String line : recording.records) {
            if (line.startsWith("sysex ")) {
                sysex.append(line.substring(6));
                if (line.endsWith("f7")) {
                    System.out.println(sysex);
                    sysex.setLength(0);
                }
            } else if (line.startsWith("out ")) {
                System.out.println(line.substring(4));
            } else if (line.equals("pause 100")) {
                pauses++;
            } else if (!line.equals("chunk 256")) {
                throw new AssertionError("Unexpected recorder event " + line);
            }
        }
        if (pauses != 1 || !sysex.isEmpty()) {
            throw new AssertionError("Unexpected pacing or incomplete SysEx");
        }
    }
}
