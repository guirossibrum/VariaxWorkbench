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
                for (String arg : args) {
                    if (arg.startsWith("name:") && name.contains(arg.substring(5))) selected.add(f);
                }
                if (!exact && (name.contains("TWXYGetParameterProperties") || name.contains("TWXYGetPatchData")
                        || name.contains("TWXYSetPatchData") || name.contains("TWXYGetPatchSize"))) {
                    selected.add(f);
                }
            }
            for (String arg : args) {
                if (arg.equals("--exact") || arg.equals("--list")) continue;
                if (arg.startsWith("name:")) continue;
                if (arg.startsWith("scalar:")) {
                    long value = Long.parseUnsignedLong(arg.substring(7), 16);
                    for (ghidra.program.model.listing.Instruction instruction : currentProgram.getListing().getInstructions(true)) {
                        for (int operand = 0; operand < instruction.getNumOperands(); operand++) {
                            for (Object object : instruction.getOpObjects(operand)) {
                                if (object instanceof ghidra.program.model.scalar.Scalar &&
                                        ((ghidra.program.model.scalar.Scalar)object).getUnsignedValue() == value) {
                                    println("SCALAR " + arg.substring(7) + " AT " + instruction.getAddress());
                                    Function owner = getFunctionContaining(instruction.getAddress());
                                    if (owner != null) selected.add(owner);
                                }
                            }
                        }
                    }
                    continue;
                }
                if (arg.startsWith("write:") || arg.startsWith("refs:")) {
                    boolean writesOnly = arg.startsWith("write:");
                    Address target = toAddr(arg.substring(writesOnly ? 6 : 5));
                    for (Reference ref : getReferencesTo(target)) {
                        println("DATAREF " + target + " FROM " + ref.getFromAddress() + " " + ref.getReferenceType());
                        if (!writesOnly || ref.getReferenceType().isWrite()) {
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
                println("FUNCTION " + f.getEntryPoint() + " " + f.getName(true));
                if (java.util.Arrays.asList(args).contains("--list")) continue;
                for (Reference ref : getReferencesTo(f.getEntryPoint())) {
                    println("XREF " + ref.getFromAddress() + " " + ref.getReferenceType());
                }
                println("CALLEES " + f.getCalledFunctions(monitor));
                for (Function callee : f.getCalledFunctions(monitor)) {
                    println("CALLEE " + callee.getEntryPoint() + " " + callee.getName(true));
                }
                DecompileResults result = decompiler.decompileFunction(f, 45, monitor);
                if (result.decompileCompleted()) println(result.getDecompiledFunction().getC());
                else println("INCOMPLETE " + result.getErrorMessage());
            }
        } finally {
            decompiler.dispose();
        }
    }
}
