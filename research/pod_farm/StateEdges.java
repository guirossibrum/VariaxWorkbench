import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.address.*;
import java.util.*;
public class StateEdges extends GhidraScript {
 public void run() throws Exception {
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  Set<Function> fs=new LinkedHashSet<>();
  for(String arg:getScriptArgs()){
   Address a=toAddr(arg);Function f=getFunctionContaining(a);println("TARGET "+a+" FUNCTION "+f);
   if(f!=null)fs.add(f);
   for(Reference r:getReferencesTo(a)){println("REF "+r);Function c=getFunctionContaining(r.getFromAddress());if(c!=null)fs.add(c);}
  }
  for(Function f:fs){println("FUNCTION "+f.getEntryPoint()+" "+f.getName());DecompileResults r=d.decompileFunction(f,45,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());else println(r.getErrorMessage());}
  d.dispose();
 }
}
