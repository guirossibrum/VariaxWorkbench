// Reference reader: use the original Workbench Java parser, with no hardware.
import cmn.l6java.patch.*;
import cmn.l6java.io.iff.L6IFFUtils;
import java.io.File;
import java.util.*;

class InspectPreset {
    public static void main(String[] args) throws Exception {
        L6Patch patch = L6PatchUtils.LoadPatch(new File(args[0]));
        if (patch == null) throw new IllegalArgumentException("not a patch");
        if (args.length == 2) {
            File output = new File(args[1]);
            if (output.exists()) throw new IllegalArgumentException("output must not exist");
            L6IFFUtils.SaveFile(output, patch);
        }
        System.out.println("name\t" + patch.GetName());
        System.out.printf("device\t%08x%n", patch.GetTargetMIDIID());
        System.out.printf("version\t%08x%n", patch.GetTargetDeviceVersion().GetInt());
        ArrayList<L6Model> models = new ArrayList<>();
        Iterator it = patch.GetModelIterator();
        while(it.hasNext()) models.add((L6Model)it.next());
        models.sort(Comparator.comparingInt(L6Model::GetSlot));
        for (L6Model model : models) {
            System.out.printf("model\t%08x\t%08x\t%d\t%b%n", model.GetSlot(), model.GetID(), model.GetOrdinal(), model.IsEnabled());
            ArrayList<L6ModelParam> params = new ArrayList<>();
            for (Object param : model.GetParams()) params.add((L6ModelParam)param);
            params.sort(Comparator.comparingInt(L6ModelParam::GetID));
            for (L6ModelParam param : params)
                System.out.printf("param\t%08x\t%06x\t%08x\t%08x%n", model.GetSlot(), param.GetID(), param.GetValueType(), param.GetValue());
        }
    }
}
