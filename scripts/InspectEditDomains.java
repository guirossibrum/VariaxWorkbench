// Offline reference helper: original parameter definitions and file reader only.
// Does not construct the editor, native coefficient engine, MIDI or USB classes.
import cmn.l6java.patch.*;
import cmn.l6java.io.iff.L6IFFUtils;
import modules.variax.device.L6VariaxParamMap;
import java.io.File;
import java.util.*;

class InspectEditDomains {
    public static void main(String[] args) throws Exception {
        if (args[0].equals("verify")) {
            File[] files = new File(args[1]).listFiles((d,n) -> n.endsWith(".vwm"));
            if (files == null || files.length == 0) throw new IllegalArgumentException("no cases");
            Arrays.sort(files);
            for (File file : files) {
                String[] fields = file.getName().replace(".vwm", "").split("-");
                int slot = Integer.parseUnsignedInt(fields[0],16);
                int id = Integer.parseUnsignedInt(fields[1],16);
                int kind = Integer.parseUnsignedInt(fields[2],16);
                int bits = Integer.parseUnsignedInt(fields[3],16);
                L6Patch patch = L6PatchUtils.LoadPatch(file);
                if (patch == null) throw new AssertionError("unreadable: " + file);
                L6ModelParam param = patch.GetModel(slot).FindParam(id,false);
                if (param == null || param.GetValueType()!=kind || param.GetValue()!=bits)
                    throw new AssertionError("parameter mismatch: " + file);
                File saved = new File(file.getParentFile(),file.getName()+".resaved");
                if (saved.exists()) throw new IllegalArgumentException("output exists");
                L6IFFUtils.SaveFile(saved,patch);
            }
            System.out.println("PASS: " + files.length + " original-reader boundary/choice cases");
            return;
        }
        L6Patch patch = L6PatchUtils.LoadPatch(new File(args[0]));
        L6VariaxParamMap map = new L6VariaxParamMap();
        int[][] groups = {
            {0x02010000,0x200000,0xff0000,1},
            {0x02010001,0x200000,131074,2},
            {0x02010001,0x200003,131076,1},
            {0x02010002,0x200000,196610,2},
            {0x02010002,0x200003,196612,1},
            {0x02010002,0x200002,196613,1},
            {0x02010002,0x200004,196614,1},
            {0x02010004,0x200000,262144,6},
            {0x02010005,0x200000,327681,19}
        };
        for (int[] group : groups) for (int i=0;i<group[3];i++) {
            L6ParamDefinition def = map.GetParamDefinition(patch,group[2]+i);
            if (def==null) throw new AssertionError("missing definition");
            ArrayList<Integer> values = new ArrayList<>();
            if (def.GetFormat() instanceof L6AssociationFormat) {
                for (Object entry : ((L6AssociationFormat)def.GetFormat()).GetAllPairsOrdered())
                    values.add(((L6AssociationFormat.ObjectValuePair)entry).value.GetBits());
            } else {
                values.add(def.GetRange().GetMin().GetBits());
                values.add(def.GetRange().GetMax().GetBits());
            }
            if (values.isEmpty()) throw new AssertionError("empty domain");
            System.out.printf("%08x\t%06x\t%d",group[0],group[1]+i,def.IsFloat()?1:0);
            for (int bits : values) System.out.printf("\t%08x",bits);
            System.out.println();
        }
    }
}
