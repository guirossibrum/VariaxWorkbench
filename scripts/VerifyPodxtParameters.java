// Offline boundary for verify_podxt_parameters.py. The vendor transcoder sees
// only an in-memory patch, a product ID and a recording L6MIDISender.
import cmn.l6java.L6SP.L6Assets;
import cmn.l6java.core.L6ValueUnionRef;
import cmn.l6java.core.L6Version;
import cmn.l6java.midi.L6MIDISender;
import cmn.l6java.patch.L6ParamDefinition;
import cmn.l6java.patch.L6Patch;
import modules.sharctone.device.L6SharcToneMIDITranscoder;
import modules.sharctone.device.podxt.L6PODxtDevice;
import modules.sharctone.device.podxt.L6PODxtParamMap;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.List;
import sun.misc.Unsafe;

public final class VerifyPodxtParameters {
    private static final int PRODUCT = 0x0003000a;
    private static final int VERSION = L6Version.ParseVersionString("3.01");
    private static final HexFormat HEX = HexFormat.of();

    private static final class DeviceId extends L6PODxtDevice {
        private DeviceId() {
            super(null, PRODUCT);
            throw new AssertionError("Device constructor must be bypassed");
        }

        @Override
        public int GetDeviceID() { return PRODUCT; }
    }

    private static DeviceId deviceId() throws ReflectiveOperationException {
        Field field = Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return (DeviceId)((Unsafe)field.get(null)).allocateInstance(DeviceId.class);
    }

    private static final class Recording implements L6MIDISender {
        final List<String> events = new ArrayList<>();
        final List<byte[]> sysex = new ArrayList<>();

        @Override
        public boolean SendCC(int controller, int value) {
            events.add("cc:" + controller + ":" + value);
            return true;
        }

        @Override
        public boolean SendSysex(byte[] bytes, boolean header, boolean product, boolean end) {
            events.add("sysex:" + (header ? 1 : 0) + (product ? 1 : 0) + (end ? 1 : 0)
                + ":" + HEX.formatHex(bytes));
            sysex.add(bytes.clone());
            return true;
        }

        @Override
        public boolean SendCC(int channel, int controller, int value) {
            throw new AssertionError("Unexpected channel-specific CC");
        }

        @Override
        public boolean SendPC(int channel, int program) {
            throw new AssertionError("Unexpected program change");
        }

        @Override
        public boolean SendPC(int program) {
            throw new AssertionError("Unexpected program change");
        }

        @Override
        public boolean SendSysex(byte[] bytes, boolean end) {
            throw new AssertionError("Unexpected implicit SysEx framing");
        }

        @Override
        public void Pause(int millis) { throw new AssertionError("Unexpected pause"); }

        @Override
        public boolean MaybeAbort() { throw new AssertionError("Unexpected abort check"); }
    }

    private final L6PODxtParamMap map = new L6PODxtParamMap(PRODUCT);
    private final L6SharcToneMIDITranscoder encoder;

    private VerifyPodxtParameters() throws ReflectiveOperationException {
        encoder = new L6SharcToneMIDITranscoder(deviceId());
        if (!encoder.IsLoadAmpDefaults()) {
            throw new AssertionError("Unexpected initial amp defaults flag");
        }
    }

    private L6Patch patch(String name, int modelSelector, int model) {
        L6Patch patch = new L6Patch(name, PRODUCT, VERSION);
        if (modelSelector != -1 && !map.SetParam(patch, modelSelector, model)) {
            throw new AssertionError("Couldn't set synthetic context model");
        }
        return patch;
    }

    private String tone(L6Patch patch, int bank, int index) {
        Recording recording = new Recording();
        if (!encoder.EncodePatch(recording, patch, map, bank, index, bank < 0 ? 1 : 2)
            || recording.sysex.size() != 2 || recording.sysex.get(1).length != 160) {
            throw new AssertionError("Expected a prefix and one 160-byte tone");
        }
        return String.join(",", recording.events);
    }

    private String parameter(String[] parts) {
        int selector = Integer.parseInt(parts[1]);
        int bits = Integer.parseUnsignedInt(parts[2]);
        int modelSelector = Integer.parseInt(parts[3]);
        int model = Integer.parseInt(parts[4]);
        encoder.SetLoadAmpDefaults(parts[5].equals("1"));

        L6Patch patch = patch("Offline", modelSelector, model);
        boolean set = map.SetParam(patch, selector, bits);

        Recording recording = new Recording();
        if (!encoder.EncodeParamChange(recording, patch, map, selector, bits, 0)) {
            throw new AssertionError("Encoder refused a parameter");
        }
        return String.join("\t", Boolean.toString(set), String.join(",", recording.events),
            tone(patch, -1, -1), tone(patch, 0, 80));
    }

    private String definition(String[] parts) {
        L6Patch patch = patch("Offline", Integer.parseInt(parts[1]), Integer.parseInt(parts[2]));
        L6ParamDefinition def = map.GetParamDefinition(patch, Integer.parseInt(parts[3]));
        return def == null ? "-" : def.GetName();
    }

    private String incoming(String[] parts) {
        byte controller = Byte.parseByte(parts[1]);
        byte value = Byte.parseByte(parts[2]);
        L6ValueUnionRef result = new L6ValueUnionRef();
        int selector = encoder.DecodeCCParamChange(null, patch("Offline", -1, 0), map,
            controller, value, result);
        return selector + ":" + Integer.toUnsignedString(result.GetBits());
    }

    private String run(String[] parts) {
        return switch (parts[0]) {
            case "P" -> parameter(parts);
            case "B" -> {
                L6Patch patch = patch("Offline", Integer.parseInt(parts[1]),
                    Integer.parseInt(parts[2]));
                yield tone(patch, -1, -1) + "\t" + tone(patch, 0, 80);
            }
            case "N" -> {
                String name = new String(HEX.parseHex(parts[1]), StandardCharsets.UTF_16BE);
                L6Patch patch = patch(name, -1, 0);
                yield tone(patch, -1, -1) + "\t" + tone(patch, 0, 80);
            }
            case "S" -> tone(patch("Offline", -1, 0), 0, Integer.parseInt(parts[1]));
            case "D" -> definition(parts);
            case "C" -> switch (parts[1]) {
                case "TAP" -> Integer.toString(L6SharcToneMIDITranscoder.CC_TAP);
                case "FX_SETUP" -> Integer.toString(L6SharcToneMIDITranscoder.CC_FX_SETUP);
                default -> throw new IllegalArgumentException("Unknown shared command");
            };
            case "A" -> L6Assets.GetAssetName(Integer.parseInt(parts[1]), PRODUCT);
            case "I" -> incoming(parts);
            case "L" -> Arrays.toString(map.GetAllParamIDs());
            default -> throw new IllegalArgumentException("Unknown fixture operation: " + parts[0]);
        };
    }

    public static void main(String[] args) throws Exception {
        VerifyPodxtParameters verifier = new VerifyPodxtParameters();
        try (BufferedReader input = new BufferedReader(
                new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            for (String line; (line = input.readLine()) != null;) {
                System.out.println(verifier.run(line.split("\t", -1)));
            }
        }
    }
}
