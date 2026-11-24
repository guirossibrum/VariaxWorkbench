// Original Line 6 Edit 3.06 classes with a fake native MIDI sender and
// scripted replies. Only synthetic patches are used. No hardware constructors run.
import cmn.l6java.core.L6Version;
import cmn.l6java.device.L6Device;
import cmn.l6java.midi.L6MIDIException;
import cmn.l6java.midi.L6MIDIManager;
import cmn.l6java.midi.L6MIDITimeoutException;
import cmn.l6java.midi.L6MIDIJava.L6MIDIJava;
import cmn.l6java.patch.L6Patch;
import cmn.l6java.patch.L6PatchManager;
import modules.sharctone.device.L6SharcToneDevice;
import modules.sharctone.device.L6SharcToneMIDITranscoder;
import modules.sharctone.device.L6SharcTonePatchManager;
import modules.sharctone.device.podxt.L6PODxtDevice;
import modules.sharctone.device.podxt.L6PODxtParamMap;

import java.io.ByteArrayOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.ArrayDeque;
import java.util.HexFormat;
import sun.misc.Unsafe;

public final class VerifyPodxtTransfer {
    static final HexFormat HEX = HexFormat.of();
    static final int PRODUCT = 0x0003000a;
    static final int VERSION = L6Version.ParseVersionString("3.01");

    private static <T> T offline(Class<T> type) throws Exception {
        Field field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return type.cast(((Unsafe) field.get(null)).allocateInstance(type));
    }

    private static void set(Class<?> type, Object object, String name, Object value) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        field.set(object, value);
    }

    private static void invoke(L6SharcTonePatchManager manager, String name,
                               Class<?>[] types, Object... args) throws Exception {
        Method method = L6SharcTonePatchManager.class.getDeclaredMethod(name, types);
        method.setAccessible(true);
        try {
            method.invoke(manager, args);
        } catch (InvocationTargetException error) {
            if (error.getCause() instanceof Exception cause) throw cause;
            throw error;
        }
    }

    private static final class Device extends L6PODxtDevice {
        private Device() { super(null, PRODUCT); throw new AssertionError("hardware constructor"); }
        @Override public int GetDeviceID() { return PRODUCT; }
    }

    private static final class Midi extends L6MIDIJava {
        private ByteArrayOutputStream wire;
        private Midi() { super(null); throw new AssertionError("hardware constructor"); }
        @Override protected void native_SendSysex(int pointer, byte[] bytes) {
            for (byte value : bytes) {
                wire.write(value);
                if ((value & 0xff) == 0xf7) {
                    System.out.println("out " + HEX.formatHex(wire.toByteArray()));
                    wire.reset();
                }
            }
        }
        @Override public void SendPC(byte channel, byte program) {
            throw new AssertionError("unexpected program change");
        }
        @Override public void SendCC(byte channel, byte controller, byte value) {
            throw new AssertionError("unexpected CC");
        }
    }

    private static final class Manager extends L6MIDIManager {
        private ArrayDeque<byte[]> replies;
        private Manager() { throw new AssertionError("hardware constructor"); }
        @Override public boolean Lock() { return true; }
        @Override public void Unlock() {}
        @Override public void SuspendMIDIDispatchThread() { System.out.println("suspend"); }
        @Override public void ResumeMIDIDispatchThread() { System.out.println("resume"); }
        @Override public byte[] WaitForSysexData(int millis) {
            System.out.println("wait " + millis);
            byte[] reply = replies.poll();
            if (reply != null) System.out.println("in " + HEX.formatHex(reply));
            return reply;
        }
        @Override public void DispatchMIDI(byte[] bytes) {
            System.out.println("dispatch " + HEX.formatHex(bytes));
        }
    }

    public static void main(String[] args) throws Exception {
        Midi midi = offline(Midi.class);
        midi.wire = new ByteArrayOutputStream();
        Manager manager = offline(Manager.class);
        manager.replies = new ArrayDeque<>();
        set(L6MIDIManager.class, manager, "m_midijava", midi);
        set(L6MIDIManager.class, manager, "m_F7", new byte[]{(byte) 0xf7});
        Field header = L6SharcToneDevice.class.getDeclaredField("SYSEX_HEADER");
        header.setAccessible(true);
        manager.SetSysexHeader(((byte[]) header.get(null)).clone());

        Device device = offline(Device.class);
        set(L6Device.class, device, "m_midi_manager", manager);
        L6PODxtParamMap map = new L6PODxtParamMap(PRODUCT);
        set(L6SharcToneDevice.class, device, "m_param_map", map);
        set(L6SharcToneDevice.class, device, "m_midi_transcoder",
            new L6SharcToneMIDITranscoder(device));

        L6SharcTonePatchManager patches = offline(L6SharcTonePatchManager.class);
        set(L6PatchManager.class, patches, "m_device", device);
        L6Patch tone = new L6Patch("Offline Tone", PRODUCT, VERSION);
        map.SetParam(tone, 65538, Float.floatToRawIntBits(0.5f));
        map.SetParam(tone, 786443, 10); // Synthetic F-T-Model 1 link field.
        set(L6PatchManager.class, patches, "m_current_patch", tone);

        for (String scenario : new String[]{"edit", "ok-16d", "ok-17a", "ok-32d", "fail", "timeout"}) {
            System.out.println("case " + scenario);
            if (scenario.equals("edit")) {
                invoke(patches, "SendDeviceEditBuffer", new Class<?>[]{L6Patch.class}, tone);
            } else {
                int index = switch (scenario) {
                    case "ok-16d" -> 63;
                    case "ok-32d" -> 127;
                    default -> 64;
                };
                if (scenario.equals("ok-17a")) manager.replies.add(HEX.parseHex("f000010c030ff7"));
                if (scenario.startsWith("ok")) manager.replies.add(HEX.parseHex("f000010c0350f7"));
                if (scenario.equals("fail")) manager.replies.add(HEX.parseHex("f000010c0351f7"));
                // The same three original methods reached by L6PatchManager.SendPatch.
                invoke(patches, "BeginSendPatchBlock", new Class<?>[]{});
                invoke(patches, "SendDevicePatch",
                    new Class<?>[]{L6Patch.class, int.class, int.class}, tone, 0, index);
                try {
                    invoke(patches, "EndSendPatchBlock", new Class<?>[]{});
                    System.out.println("result ok");
                } catch (L6MIDITimeoutException error) {
                    System.out.println("result timeout");
                } catch (L6MIDIException error) {
                    System.out.println("result failure");
                }
            }
            if (midi.wire.size() != 0 || !manager.replies.isEmpty()) {
                throw new AssertionError("incomplete offline transcript");
            }
        }
    }
}
