"""Run a JSON list of MCP tool calls through one locally built REA process.

Each step is {"tool": name, "arguments": {...}, "output": file}. Results are
preserved verbatim. Tool errors stop the batch. No application target is executed
unless the job explicitly requests an execution tool.
"""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    process = subprocess.Popen([str(ROOT / "scripts/rea"), "mcp"],
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=sys.stderr, text=True, bufsize=1)
    request_id = 0

    def request(method, params):
        nonlocal request_id
        request_id += 1
        process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": request_id,
                                       "method": method, "params": params}) + "\n")
        process.stdin.flush()
        while line := process.stdout.readline():
            response = json.loads(line)
            if response.get("id") == request_id:
                if "error" in response:
                    raise RuntimeError(response["error"])
                return response["result"]
        raise RuntimeError(f"REA closed stdout (exit {process.poll()})")

    try:
        request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                               "clientInfo": {"name": "variax-offline-analysis", "version": "1"}})
        process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        process.stdin.flush()
        if sys.argv[1] == "--catalog":
            result = request("tools/list", {})
            Path(sys.argv[2]).write_text(json.dumps(result, indent=2) + "\n")
            print(f"Saved {len(result['tools'])} tool schemas")
        else:
            for step in json.loads(Path(sys.argv[1]).read_text()):
                print(f"Running {step['tool']}", flush=True)
                result = request("tools/call", {"name": step["tool"],
                                               "arguments": step.get("arguments", {})})
                output = Path(step["output"])
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_text(json.dumps(result, indent=2) + "\n")
                print(f"Saved {output}", flush=True)
                if result.get("isError"):
                    raise RuntimeError(f"Tool failed; see {output}")
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=10)


if __name__ == "__main__":
    main()
