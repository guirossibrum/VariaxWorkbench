// Execute original Java mapping, ApplyHacks and final overlay with a fake engine.
// No device constructors or native entry points are executed.
import cmn.l6java.patch.*;
import cmn.l6java.core.*;
import modules.variax.device.*;
import modules.variax.device.coefeng.L6VariaxCoefEngine;
import java.lang.reflect.*;
import java.nio.*;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import sun.misc.Unsafe;

public class VerifyHostPipeline {
    static List<String> layout;
    static int seed;
    static byte[] mapped;
    static int mask;
    static class MapInput extends L6ParamMap {
        Map<Integer,Integer> values = new HashMap<>();
        public boolean SetParam(L6Patch p,int id,int bits) { values.put(id,bits);return true; }
        public int GetParam(L6Patch p,int id) { return values.getOrDefault(id,0); }
        public L6ParamDefinition GetParamDefinition(L6Patch p,int id) { throw new AssertionError(); }
        public boolean GetParam(L6Patch p,int id,L6ValueUnionRef v,L6ObjectRef d) { throw new AssertionError(); }
    }
    static class FakeEngine extends L6VariaxCoefEngine {
        public byte[] CreateHWPatch(VWPatch p) {
            try {
                Method hacks=L6VariaxCoefEngine.class.getDeclaredMethod("ApplyHacks",VWPatch.class);
                hacks.setAccessible(true);hacks.invoke(this,p);
                mapped=new byte[192];ByteBuffer out=ByteBuffer.wrap(mapped).order(ByteOrder.LITTLE_ENDIAN);
                for(String line:layout) {
                    String[] f=line.split(" ");Field field=VWPatch.class.getField(f[0]);int offset=Integer.parseInt(f[1]);
                    if(f[2].equals("Z")) mapped[offset]=(byte)(field.getBoolean(p)?1:0);
                    else out.putInt(offset,f[2].equals("F")?Float.floatToRawIntBits(field.getFloat(p)):field.getInt(p));
                }
                mask=p.m_block_mask;
                byte[] packed=new byte[3276];int state=seed;
                for(int i=0;i<packed.length;i++) {state=state*1664525+1013904223;packed[i]=(byte)(state>>>24);}
                return packed;
            } catch(Exception e) {throw new RuntimeException(e);}
        }
    }
    public static void main(String[] args) throws Exception {
        layout=Files.readAllLines(Path.of(args[1]));
        Field access=Unsafe.class.getDeclaredField("theUnsafe");access.setAccessible(true);
        Unsafe unsafe=(Unsafe)access.get(null);
        L6VariaxMIDITranscoder transcoder=(L6VariaxMIDITranscoder)unsafe.allocateInstance(L6VariaxMIDITranscoder.class);
        Field engine=L6VariaxMIDITranscoder.class.getDeclaredField("m_coefeng");engine.setAccessible(true);engine.set(transcoder,new FakeEngine());
        Method generate=L6VariaxMIDITranscoder.class.getDeclaredMethod("GeneratePatch",L6Patch.class,L6ParamMap.class,boolean.class);generate.setAccessible(true);
        HexFormat hex=HexFormat.of();
        for(String line:Files.readAllLines(Path.of(args[0]))) {
            String[] f=line.split(" ");String name=new String(hex.parseHex(f[0]),StandardCharsets.UTF_8);
            int flags=Integer.parseInt(f[1]);L6VariaxDevice.ENABLE_ALL_12STR=(flags&1)!=0;
            L6VariaxMIDITranscoder.OPT_BLOCK_MASK=(int)Long.parseLong(f[2],16);seed=(int)Long.parseLong(f[3],16);
            MapInput values=new MapInput();
            for(int i=4;i<f.length;i++) {String[] pair=f[i].split(":");values.values.put((int)Long.parseLong(pair[0],16),(int)Long.parseLong(pair[1],16));}
            byte[] result=(byte[])generate.invoke(transcoder,new L6Patch(name,0x70004,0x310),values,(flags&2)!=0);
            System.out.println(line+" => "+hex.formatHex(mapped)+" "+Integer.toUnsignedString(mask,16)+" "+hex.formatHex(result));
        }
    }
}
