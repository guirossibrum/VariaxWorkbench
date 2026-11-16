// Offline original Java decoder oracle. No device constructor, JNI, MIDI or UI.
// Input slot files were extracted from an archived firmware image, not captured
// from a guitar. Output is model/settings data, not a calibration backup.
import cmn.l6java.patch.L6Patch;
import cmn.l6java.io.iff.L6IFFUtils;
import modules.variax.device.L6VariaxMIDITranscoder;
import modules.variax.device.L6VariaxParamMap;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import sun.misc.Unsafe;

class DecodeFirmwareReference {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) throw new IllegalArgumentException("EXTRACTED_DIRECTORY NEW_OUTPUT_DIRECTORY");
        Path source = Path.of(args[0]);
        Path output = Path.of(args[1]);
        Files.createDirectory(output); // Reject any existing destination.
        Field accessor = Unsafe.class.getDeclaredField("theUnsafe");
        accessor.setAccessible(true);
        Unsafe unsafe = (Unsafe)accessor.get(null);
        L6VariaxMIDITranscoder decoder = (L6VariaxMIDITranscoder)unsafe.allocateInstance(L6VariaxMIDITranscoder.class);
        for (String field : new String[]{"m_deviceID", "m_deviceVersion"}) {
            Field f = L6VariaxMIDITranscoder.class.getDeclaredField(field);
            f.setAccessible(true);
            f.setInt(decoder, field.equals("m_deviceID") ? 0x00070004 : 0x03100000);
        }
        for (int slot = 0; slot < 60; slot++) {
            Path file = source.resolve(String.format("slot-%02d.u24le", slot));
            if (!Files.isRegularFile(file) || Files.size(file) != 4096)
                throw new IllegalArgumentException("expected a regular 4096-byte slot file");
            byte[] raw = Files.readAllBytes(file);
            byte[] payload = new byte[384];
            // Present only metadata to the pure DecodePatch routine, using the
            // nibble order its existing parser expects. Nothing is transmitted.
            for (int word = 0; word < 64; word++) {
                int at = 2553 + word * 3;
                int value = (raw[at] & 255) | ((raw[at+1] & 255) << 8) | ((raw[at+2] & 255) << 16);
                for (int nibble = 0; nibble < 6; nibble++)
                    payload[word*6+nibble] = (byte)((value >>> (4*nibble)) & 15);
            }
            L6Patch patch = decoder.DecodePatch(new L6VariaxParamMap(), payload);
            Path destination = output.resolve(String.format("slot-%02d.vwm", slot));
            L6IFFUtils.SaveFile(destination.toFile(), patch);
            if (!Files.isRegularFile(destination)) throw new IllegalStateException("Java save failed");
            System.out.printf("%02d\t%s%n", slot, patch.GetName());
        }
    }
}
