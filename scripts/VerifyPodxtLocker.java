// Synthetic PODxt tone locker fixtures. Only file codecs run in this process.
import cmn.l6java.core.L6Version;
import cmn.l6java.io.iff.L6IFFUtils;
import cmn.l6java.patch.*;
import cmn.l6java.midi.L6MIDISender;
import modules.sharctone.device.L6SharcToneMIDITranscoder;
import modules.sharctone.device.podxt.*;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HexFormat;
import sun.misc.Unsafe;

public final class VerifyPodxtLocker {
    static final int PRODUCT = 0x3000a;
    static final int VERSION = L6Version.ParseVersionString("3.01");
    static final HexFormat HEX = HexFormat.of();

    static class OfflineDevice extends L6PODxtDevice {
        OfflineDevice() { super(null, PRODUCT); throw new AssertionError("hardware constructor"); }
        public int GetDeviceID() { return PRODUCT; }
        public int GetDeviceVersion() { return VERSION; }
    }

    static class Recording implements L6MIDISender {
        byte[] payload;
        public boolean SendSysex(byte[] bytes, boolean header, boolean product, boolean end) {
            if (end) payload = bytes.clone();
            return true;
        }
        public boolean SendSysex(byte[] bytes, boolean end) { throw new AssertionError(); }
        public boolean SendCC(int controller, int value) { throw new AssertionError(); }
        public boolean SendCC(int channel, int controller, int value) { throw new AssertionError(); }
        public boolean SendPC(int channel, int program) { throw new AssertionError(); }
        public boolean SendPC(int program) { throw new AssertionError(); }
        public void Pause(int millis) { throw new AssertionError(); }
        public boolean MaybeAbort() { throw new AssertionError(); }
    }

    public static void main(String[] args) throws Exception {
        byte[] frame = new byte[168];
        byte[] prefix = HEX.parseHex("f000010c03740a");
        System.arraycopy(prefix, 0, frame, 0, prefix.length);
        Arrays.fill(frame, 7, 39, (byte) 32);
        byte[] name = "Locker fixture".getBytes(StandardCharsets.US_ASCII);
        System.arraycopy(name, 0, frame, 7, name.length);
        frame[39 + 13] = 64;
        frame[39 + 57] = 127;
        frame[39 + 65] = 64;
        frame[39 + 89] = 9;
        frame[39 + 90] = 48;
        frame[39 + 120] = 10;
        frame[167] = (byte) 0xf7;

        if (args.length == 2 && args[0].equals("check")) {
            Object loaded = L6PatchUtils.LoadPatchObject(new File(args[1]));
            if (!(loaded instanceof L6Bundle bundle) || bundle.NumBanks() != 3 || !bundle.IsFixed()) {
                throw new AssertionError("expected 3-bank L6BA");
            }
            int[] sizes = {128, 107, 64};
            int[] types = {0, 2, 1};
            var map = new L6PODxtParamMap(PRODUCT);
            for (int i = 0; i < sizes.length; ++i) {
                L6PatchBank bank = bundle.GetBank(i);
                if (!bank.IsFixed() || bank.NumPatches() != sizes[i]) throw new AssertionError("bank size");
                for (int slot = 0; slot < sizes[i]; ++slot) {
                    L6Patch tone = bank.GetPatch(slot);
                    if (tone.GetType() != types[i] || tone.GetTargetMIDIID() != PRODUCT
                        || !"Locker fixture".equals(tone.GetName())
                        || map.GetParamValueFloat(tone, 65538) != 64.0f / 127.0f
                        || map.GetParamValueInt(tone, 786441) != 10) {
                        throw new AssertionError("tone fields at " + i + ":" + slot);
                    }
                }
            }
            System.out.println("Original Line 6 Edit loaded all 299 generated entries and checked tone fields.");
            return;
        }

        Field field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        var device = (OfflineDevice) ((Unsafe) field.get(null)).allocateInstance(OfflineDevice.class);
        var encoder = new L6SharcToneMIDITranscoder(device);
        var map = new L6PODxtParamMap(PRODUCT);
        if (args.length == 1 && args[0].equals("defaults")) {
            var tone = L6PatchUtils.LoadPatch(L6PODxtDevice.class.getResourceAsStream("Default Tone.l6t"));
            var recording = new Recording();
            encoder.EncodePatch(recording, tone, map, -1, -1, 1);
            for (int cc = 0; cc < 128; ++cc) {
                int value = recording.payload[32 + cc] & 0xff;
                if (value != 0) System.out.printf("(%d, %d),%n", cc, value);
            }
            return;
        }
        var bundle = new L6Bundle(true);
        String[] names = {"Tones", "Amp Setups", "FX Setups"};
        for (int i = 0; i < names.length; ++i) {
            var bank = new L6PatchBank(names[i], 0, true);
            L6Patch tone = encoder.DecodePatch(map, frame);
            tone.SetType(new int[]{0, 2, 1}[i]);
            bank.SetPatch(0, tone);
            bundle.AddBank(bank);
        }
        L6IFFUtils.SaveFile(new File(args[0]), bundle);
        System.out.println(HEX.formatHex(frame));
    }
}
