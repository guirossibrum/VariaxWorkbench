// Offline Ghidra research script; results contain private original-derived code.
// Run only against copied, hash-verified binaries. Never execute vendor code.
// @category VariaxWorkbench.Research
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.listing.Function;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.Reference;
import java.util.LinkedHashSet;
import java.util.Set;

public class PodFarmEdge extends GhidraScript {
    @Override
    public void run() throws Exception {
        DecompInterface decompiler = new DecompInterface();
        try {
            decompiler.openProgram(currentProgram);
            Set<Function> selected = new LinkedHashSet<>();
            String[] args = getScriptArgs();
            boolean exact = java.util.Arrays.asList(args).contains("--exact");
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                String name = f.getName();
                if (!exact && (name.contains("TWXYGetParameterProperties") || name.contains("TWXYGetPatchData")
                        || name.contains("TWXYSetPatchData") || name.contains("TWXYGetPatchSize"))) {
                    selected.add(f);
                }
            }
            for (String arg : args) {
                if (arg.equals("--exact")) continue;
                if (arg.startsWith("write:")) {
                    Address target = toAddr(arg.substring(6));
                    for (Reference ref : getReferencesTo(target)) {
                        println("DATAREF " + target + " FROM " + ref.getFromAddress() + " " + ref.getReferenceType());
                        if (ref.getReferenceType().isWrite()) {
                            Function owner = getFunctionContaining(ref.getFromAddress());
                            if (owner != null) selected.add(owner);
                        }
                    }
                    continue;
                }
                Address address = toAddr(arg);
                Function f = getFunctionContaining(address);
                if (f == null) {
                    disassemble(address);
                    f = createFunction(address, null);
                }
                if (f != null) selected.add(f);
            }
            for (Function f : new LinkedHashSet<>(selected)) {
                if (!exact) selected.addAll(f.getCalledFunctions(monitor));
            }
            println("PROGRAM " + currentProgram.getName() + " LANGUAGE " + currentProgram.getLanguageID());
            for (Function f : selected) {
                monitor.checkCancelled();
                println("FUNCTION " + f.getEntryPoint() + " " + f.getName());
                for (Reference ref : getReferencesTo(f.getEntryPoint())) {
                    println("XREF " + ref.getFromAddress() + " " + ref.getReferenceType());
                }
                println("CALLEES " + f.getCalledFunctions(monitor));
                DecompileResults result = decompiler.decompileFunction(f, 45, monitor);
                if (result.decompileCompleted()) println(result.getDecompiledFunction().getC());
                else println("INCOMPLETE " + result.getErrorMessage());
            }
        } finally {
            decompiler.dispose();
        }
    }
}
