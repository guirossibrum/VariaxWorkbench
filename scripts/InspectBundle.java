// Offline reference harness: original Java bundle writer and reader, no UI/JNI.
import cmn.l6java.patch.*;
import cmn.l6java.io.iff.L6IFFUtils;
import java.io.File;
class InspectBundle {
    public static void main(String[] args) throws Exception {
        L6Bundle bundle;
        if (args[0].equals("create") || args[0].equals("create-matrix")) {
            bundle = new L6Bundle(Boolean.parseBoolean(args[3]));
            int[] counts = args[0].equals("create-matrix") ? new int[]{0,2,3} : new int[]{1};
            for (int i=0;i<counts.length;i++) {
                L6PatchBank bank = new L6PatchBank("Reference 🎸 " + i);
                bank.SetFixed(bundle.IsFixed());
                for (int j=0;j<counts[i];j++) {
                    L6Patch patch = L6PatchUtils.LoadPatch(new File(args[1]));
                    if (args[0].equals("create-matrix")) patch.SetName("Preset " + i + ":" + j);
                    bank.AddPatch(patch);
                }
                bundle.AddBank(bank);
            }
            File output = new File(args[2]);
            if (output.exists()) throw new IllegalArgumentException("output exists");
            L6IFFUtils.SaveFile(output, bundle);
        } else {
            bundle = (L6Bundle)L6PatchUtils.LoadPatchObject(new File(args[1]));
            if (args[0].equals("resave")) {
                File output = new File(args[2]);
                if (output.exists()) throw new IllegalArgumentException("output exists");
                L6IFFUtils.SaveFile(output,bundle);
            }
        }
        System.out.printf("bundle\t%s\t%d banks%n", bundle.IsFixed() ? "fixed" : "collection", bundle.NumBanks());
        for (int i=0; i<bundle.NumBanks(); i++) {
            L6PatchBank bank = bundle.GetBank(i);
            System.out.printf("bank\t%d\t%s\t%d presets%n", i, bank.GetName(), bank.NumPatches());
            for (int j=0; j<bank.NumPatches(); j++) {
                L6Patch preset = bank.GetPatch(j);
                System.out.printf("preset\t%d\t%d\t%s\t%08x%n", i,j,preset.GetName(),preset.GetTargetMIDIID());
            }
        }
    }
}
