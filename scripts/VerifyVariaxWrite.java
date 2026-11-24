// Runs original Workbench classes with an offline MIDI recording boundary.
// Hardware constructors are bypassed. Only the separate coefficient oracle
// executes vendor machine instructions, in CPU emulation.
import cmn.l6java.device.L6Device;
import cmn.l6java.midi.L6MIDIManager;
import cmn.l6java.midi.L6MIDIJava.L6MIDIJava;
import cmn.l6java.patch.*;
import cmn.l6java.utils.L6StaticUtils;
import modules.variax.device.*;
import modules.variax.device.coefeng.L6VariaxCoefEngine;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import sun.misc.Unsafe;

public class VerifyVariaxWrite {
    static final HexFormat HEX = HexFormat.of();
    static final int WORD_BYTES = 3276;
    static final byte[] ACK = HEX.parseHex("f000010c070071f7");
    static final byte[] BANK_NAME = HEX.parseHex("f000010c07006d437573746f6d2062616e6b00000000000000f7");
    static final Unsafe UNSAFE;

    static {
        try {
            Field field = Unsafe.class.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            UNSAFE = (Unsafe) field.get(null);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    static <T> T allocate(Class<T> type) throws InstantiationException {
        return type.cast(UNSAFE.allocateInstance(type));
    }

    static void set(Class<?> type, Object object, String name, Object value) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        field.set(object, value);
    }

    static Object call(Class<?> type, Object object, String name, Class<?>[] signature,
                       Object... args) throws Exception {
        Method method = type.getDeclaredMethod(name, signature);
        method.setAccessible(true);
        return method.invoke(object, args);
    }

    static class RecordedMidi extends L6MIDIJava {
        List<String> records;
        RecordedMidi() { super(null); throw new AssertionError(); }
        public PortState GetPortState() { return null; }
        protected void native_SetSysexChunkSize(int pointer, int size) {
            records.add("chunk " + size);
        }
        protected void native_SendSysex(int pointer, byte[] bytes) {
            records.add("sysex " + HEX.formatHex(bytes));
        }
        public void SendCC(byte channel, byte controller, byte value) {
            records.add("out " + HEX.formatHex(new byte[]{(byte) (0xb0 | channel), controller, value}));
        }
        public void SendPC(byte channel, byte program) {
            records.add("out " + HEX.formatHex(new byte[]{(byte) (0xc0 | channel), program}));
        }
    }

    static class Manager extends L6MIDIManager {
        byte[] reply;
        int wait;
        Manager() { throw new AssertionError(); }
        public boolean Lock() { return true; }
        public void Unlock() {}
        public void SuspendMIDIDispatchThread() {}
        public void ResumeMIDIDispatchThread() {}
        public byte[] WaitForMIDIData(int timeout) {
            if (timeout != wait || reply == null) throw new AssertionError("Unexpected reply wait");
            byte[] result = reply;
            reply = null;
            return result;
        }
    }

    static class Device extends L6VariaxDevice {
        Device() { super(null, 0); throw new AssertionError(); }
        public int GetConnectionMode(boolean update) { return 1; }
        public void ResumeConnectionPolling() {}
    }

    static class Engine extends L6VariaxCoefEngine {
        byte[] normal;
        byte[] muted;
        int slot;
        public byte[] CreateHWPatch(VWPatch patch) {
            byte[] source = patch.m_trim == -1000.0f ? muted : normal;
            return Arrays.copyOfRange(source, slot * WORD_BYTES, (slot + 1) * WORD_BYTES);
        }
    }

    static void end(Device device, L6VariaxPatchManager patches, Manager midi, boolean block)
        throws Exception {
        if (block) {
            call(L6VariaxPatchManager.class, patches, "EndSendPatchBlock", new Class<?>[]{});
        } else {
            device.SetDSPEnabled(true);
        }
        // Rust closes the short-lived MIDI session explicitly. The original
        // OnClose emits DSP on and bridge release without sending session off.
        midi.SendSysex(new byte[]{0x2a, 0}, true);
        call(L6VariaxDevice.class, device, "SetXTLiveVariaxMessagesEnable",
             new Class<?>[]{boolean.class}, false);
    }

    static void begin(Device device, L6VariaxPatchManager patches, RecordedMidi midi, boolean block)
        throws Exception {
        midi.records.clear();
        device.OnInit();
        if (block) call(L6VariaxPatchManager.class, patches, "BeginSendPatchBlock", new Class<?>[]{});
    }

    public static void main(String[] args) throws Exception {
        Path directory = Path.of(args[0]);
        Engine engine = new Engine();
        engine.normal = Files.readAllBytes(directory.resolve("normal/native.bin"));
        engine.muted = Files.readAllBytes(directory.resolve("muted/native.bin"));
        RecordedMidi recording = allocate(RecordedMidi.class);
        recording.records = new ArrayList<>();
        L6StaticUtils.recordSleep = millis -> recording.records.add("pause " + millis);

        Manager midi = allocate(Manager.class);
        set(L6MIDIManager.class, midi, "m_midijava", recording);
        set(L6MIDIManager.class, midi, "m_F7", new byte[]{(byte) 0xf7});
        Device device = allocate(Device.class);
        set(L6Device.class, device, "m_midi_manager", midi);
        L6VariaxPatchManager patches = allocate(L6VariaxPatchManager.class);
        set(L6PatchManager.class, patches, "m_device", device);
        set(L6VariaxDevice.class, device, "m_patch_manager", patches);
        set(L6VariaxPatchManager.class, patches, "WRITE_PATCH_MSG", new byte[]{0x61, 0});
        set(L6VariaxPatchManager.class, patches, "GET_CURRENT_BANK_NAME_MSG", new byte[]{0x6d});
        byte[] bankMessage = new byte[19];
        bankMessage[0] = 0x6c;
        set(L6VariaxPatchManager.class, patches, "SET_CURRENT_BANK_NAME_MSG", bankMessage);

        var transcoder = allocate(L6VariaxMIDITranscoder.class);
        set(L6VariaxMIDITranscoder.class, transcoder, "m_coefeng", engine);
        set(L6VariaxMIDITranscoder.class, transcoder, "m_update_guitar_eb_data", true);
        set(L6VariaxDevice.class, device, "m_midi_transcoder", transcoder);
        var map = new L6VariaxParamMap();
        set(L6VariaxDevice.class, device, "m_param_map", map);
        var names = new ArrayList<String>();

        for (int slot = 0; slot < 60; ++slot) {
            engine.slot = slot;
            Path presets = directory.resolve("presets");
            L6Patch patch = L6PatchUtils.LoadPatch(presets.resolve(slot + ".vwm").toFile());
            names.add(patch.GetName());
            int previous = (slot + 59) % 60;
            L6Patch previousPatch = L6PatchUtils.LoadPatch(presets.resolve(previous + ".vwm").toFile());
            begin(device, patches, recording, false);
            transcoder.EncodePatch(midi, patch, map, -1, -1, 0);
            transcoder.EncodeParamChange(midi, patch, map, 0x40002, 0, 0);
            engine.slot = previous;
            transcoder.EncodeParamChange(midi, previousPatch, map, 0x40002, 0, 0);
            engine.slot = slot;
            transcoder.EncodeParamChange(midi, patch, map, 0x2050000, 0, 0);
            transcoder.EncodeParamChange(midi, patch, map, 0x40004, Float.floatToIntBits(0.5f), 0);
            end(device, patches, midi, false);
            Files.write(directory.resolve("java-" + slot + "-edit.txt"), recording.records);

            begin(device, patches, recording, true);
            midi.reply = ACK;
            midi.wait = 15000;
            call(L6VariaxPatchManager.class, patches, "SendDevicePatch",
                 new Class<?>[]{L6Patch.class, int.class, int.class}, patch, slot / 5, slot % 5);
            if (midi.reply != null) throw new AssertionError("Store did not read its reply");
            end(device, patches, midi, true);
            Files.write(directory.resolve("java-" + slot + "-store.txt"), recording.records);

            begin(device, patches, recording, true);
            call(L6VariaxPatchManager.class, patches, "SendBankName",
                 new Class<?>[]{int.class, String.class}, slot / 5, "Custom bank");
            midi.reply = BANK_NAME;
            midi.wait = 1000;
            Object name = call(L6VariaxPatchManager.class, patches, "RequestBankName",
                               new Class<?>[]{int.class}, -1);
            if (!"Custom bank".equals(name)) throw new AssertionError("Bank name readback");
            end(device, patches, midi, true);
            Files.write(directory.resolve("java-" + slot + "-bank.txt"), recording.records);
        }
        Files.write(directory.resolve("names.txt"), names);
    }
}
