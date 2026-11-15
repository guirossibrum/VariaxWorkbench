// Offline reference isolation: never load an original native/OS library.
package cmn.l6java.utils;
public class L6StaticUtils {
    public static String ZeroTerminate(String value) { int i=value.indexOf('\0');return i<0?value:value.substring(0,i); }
    public static boolean IsWindows() { return false; }
    public static boolean LoadLibrary(Class<?> type, String path) { return true; }
}
