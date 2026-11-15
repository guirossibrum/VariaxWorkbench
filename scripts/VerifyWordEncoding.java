// Runs only the original application's static word encoder. No device is opened.
import modules.variax.device.L6VariaxDevice;
import java.util.HexFormat;

class VerifyWordEncoding {
    public static void main(String[] args) {
        for (int value : new int[]{0, 32, 851, 883, 0x123456, 0xffffff}) {
            byte[] bytes = new byte[6];
            L6VariaxDevice.Int2VXWord(bytes, 0, value);
            System.out.println(value + " " + HexFormat.of().formatHex(bytes));
        }
    }
}
