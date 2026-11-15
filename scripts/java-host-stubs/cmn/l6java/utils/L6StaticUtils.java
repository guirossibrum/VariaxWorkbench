// Offline reference isolation: never load an original native/OS library.
package cmn.l6java.utils;
public class L6StaticUtils {
    public static boolean IsWindows() { return false; }
    public static boolean LoadLibrary(Class<?> type, String path) { return true; }
}
