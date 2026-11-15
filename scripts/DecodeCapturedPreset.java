// Original Workbench metadata decoder, isolated from its device constructor.
// Only pure DecodePatch runs; no JNI library, UI, USB or MIDI port is opened.
import cmn.l6java.patch.L6Patch;
import cmn.l6java.io.iff.L6IFFUtils;
import modules.variax.device.L6VariaxMIDITranscoder;
import modules.variax.device.L6VariaxParamMap;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import sun.misc.Unsafe;

class DecodeCapturedPreset {
    public static void main(String[] args) throws Exception {
        byte[] first = Files.readAllBytes(Path.of(args[0]));
        byte[] second = Files.readAllBytes(Path.of(args[1]));
        if (first.length != 200 || second.length != 200) throw new IllegalArgumentException("expected two metadata replies");
        byte[] payload = new byte[384];
        System.arraycopy(first, 7, payload, 0, 192);
        System.arraycopy(second, 7, payload, 192, 192);
        Field accessor = Unsafe.class.getDeclaredField("theUnsafe");
        accessor.setAccessible(true);
        Unsafe unsafe = (Unsafe)accessor.get(null);
        L6VariaxMIDITranscoder decoder = (L6VariaxMIDITranscoder)unsafe.allocateInstance(L6VariaxMIDITranscoder.class);
        for (String field : new String[]{"m_deviceID", "m_deviceVersion"}) {
            Field f = L6VariaxMIDITranscoder.class.getDeclaredField(field);
            f.setAccessible(true);
            f.setInt(decoder, field.equals("m_deviceID") ? 0x00070004 : 0x03100000);
        }
        L6Patch patch = decoder.DecodePatch(new L6VariaxParamMap(), payload);
        File output = new File(args[2]);
        if (output.exists()) throw new IllegalArgumentException("output must not exist");
        L6IFFUtils.SaveFile(output, patch);
        System.out.println(patch.GetName());
    }
}
