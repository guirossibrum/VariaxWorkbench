#!/usr/bin/env python3
"""Observe Herdr completions across Git repos and route configured reports."""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import uuid

READY = {"idle", "done"}
CAN_PROMPT = READY | {"working"}
MAX_BATCH_BYTES = 64 * 1024
LEGACY_REVIEW_HOLD = "Owner review in the dedicated review pane"
HERDR_ERRORS = (RuntimeError, OSError, subprocess.TimeoutExpired, json.JSONDecodeError)
HEADINGS = ["Result", "Verification", "Remaining work", "Worktree handoff"]
TAIL = re.compile(r"^(?:[›❯✶]|Worked for \d|[└╰]─)")
TABLE_TOP = re.compile(r"┌─+(?:┬─+)*┐")
TABLE_RULE = re.compile(r"├─+(?:┼─+)*┤")
TABLE_BOTTOM = re.compile(r"└─+(?:┴─+)*┘")


def report_tail(text):
    """Keep closed table borders inside the report and stop at terminal endings."""
    columns = 0
    offset = 0
    has_rows = ruled = False
    for raw_line in text.splitlines(keepends=True):
        line_start = offset
        offset += len(raw_line)
        line = raw_line.strip()
        if columns:
            if len(line) >= 2 and line[0] == line[-1] == "│":
                has_rows = True
                continue
            if TABLE_RULE.fullmatch(line) and line.count("┼") == columns - 1:
                ruled = True
                continue
            if columns > 1 or ruled:
                if not (has_rows and TABLE_BOTTOM.fullmatch(line)
                        and line.count("┴") == columns - 1):
                    return None
                columns = 0
                continue
            columns = 0
        if TAIL.match(line):
            return text[:line_start]
        if TABLE_TOP.fullmatch(line):
            columns = line.count("┬") + 1
            has_rows = ruled = False
    if columns:
        return None
    return text


def report_from_screen(screen, lane, binding=None):
    # Grok can render the headings and right-align a timestamp on the title.
    binding = binding or {}
    label = binding.get("heading", f"## Lane #{lane}")
    expected = binding.get("sections", HEADINGS)
    rendered = re.sub(r"^#{1,6} +", "", label)
    timestamp = r"(?:[ \t]{2,}(?:[1-9]|1[0-2]):[0-5][0-9] (?:AM|PM))?"
    title = re.compile(
        rf"^[ \t]*(?:[•●][ \t]*)?(?:({re.escape(label)})|"
        rf"{re.escape(rendered)}{timestamp})[ \t]*$", re.M,
    )
    matches = list(title.finditer(screen))
    if not matches:
        return None
    match = matches[-1]
    tail = report_tail(screen[match.end():])
    if tail is None or len(tail) > 12_000:
        return None
    lines = [line.strip() for line in tail.strip().splitlines()]
    headings = []
    for index, line in enumerate(lines):
        if line.startswith("### "):
            headings.append((index, line[4:]))
        elif line in expected and (match[1] != label or rendered == label):
            headings.append((index, line))
            lines[index] = f"### {line}"
    if [name for _, name in headings] != expected:
        return None
    positions = [index for index, _ in headings] + [len(lines)]
    if any(not any(lines[start + 1:end]) for start, end in zip(positions, positions[1:])):
        return None
    return f"{label}\n\n" + "\n".join(lines)


def digest(report):
    return hashlib.sha256(report.encode()).hexdigest()


def write_state(path, state):
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent, delete=False) as f:
        temporary = Path(f.name)
        json.dump(state, f, indent=2)
        f.flush()
        os.fsync(f.fileno())
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


class Herdr:
    def __init__(self, config, repository=None):
        self.command = [config["herdr"], "--session", config["session"], "agent"]
        self.repo = Path(repository).resolve() if repository else None

    def run(self, *args):
        try:
            result = subprocess.run(
                [*self.command, *args], text=True, encoding="utf-8",
                capture_output=True, timeout=15,
            )
        except subprocess.TimeoutExpired:
            raise RuntimeError(f"Herdr {args[0]} timed out") from None
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or f"Herdr {args[0]} failed")
        return result.stdout

    def agents(self):
        return {agent["pane_id"]: agent for agent in json.loads(self.run("list"))["result"]["agents"]}

    def match(self, agents, binding, gatekeeper=False):
        agent = agents.get(binding["pane"])
        if not agent or agent.get("agent_session") != binding["session"]:
            return None
        location = Path(agent.get("foreground_cwd") or "/").resolve()
        if gatekeeper and location != self.repo:
            return None
        if location == self.repo or repository_for(location) == self.repo:
            return agent
        return None

    def read(self, pane, visible=False):
        if visible:
            return self.run("read", pane, "--source", "visible")
        return self.run("read", pane, "--source", "recent-unwrapped", "--lines", "200")

    def prompt(self, pane, text):
        response = json.loads(self.run("prompt", pane, text))
        result = response.get("result", {})
        if result.get("type") != "agent_prompted":
            raise RuntimeError("Herdr did not acknowledge the prompt")
        return result.get("agent")

    def submit(self, binding, text):
        gate = self.match(self.agents(), binding, gatekeeper=True)
        if not gate or gate["agent_status"] not in CAN_PROMPT:
            return False
        # Ordinary agent prompt: no interrupt, raw keys, or lifecycle wait.
        recipient = self.prompt(binding["pane"], text)
        if (not isinstance(recipient, dict) or recipient.get("pane_id") != binding["pane"]
                or recipient.get("agent_session") != binding["session"]):
            raise RuntimeError("Herdr acknowledgement did not match the bound Gatekeeper")
        agents = {binding["pane"]: recipient} if recipient.get("foreground_cwd") else self.agents()
        if not self.match(agents, binding, gatekeeper=True):
            raise RuntimeError("Herdr acknowledgement did not match the bound Gatekeeper")
        return True


def baseline(herdr, config):
    agents = herdr.agents()
    if not herdr.match(agents, config["gatekeeper"], gatekeeper=True):
        raise ValueError("Gatekeeper binding does not match a live session in this repository")
    state = {"seen": {}, "pending": [], "lane_bindings": config["lanes"]}
    for lane, binding in config["lanes"].items():
        agent = herdr.match(agents, binding)
        if not agent:
            raise ValueError(f"Lane #{lane} binding does not match a live session")
        screen = herdr.read(binding["pane"], visible=agent["agent_status"] not in READY)
        report = report_from_screen(screen, lane, binding)
        state["seen"][lane] = {"sequence": agent.get("completion_seq") or 0,
                               "digest": digest(report) if report else None}
    return state


def capture(herdr, config, state, save):
    if "lane_bindings" not in state:
        # Migrate the prior state format in place and retain pending reports.
        if set(state["seen"]) != set(config["lanes"]):
            raise RuntimeError("Configured lanes changed; preserve pending reports before rebinding")
        state["lane_bindings"] = config["lanes"]
        save(state)
    elif state["lane_bindings"] != config["lanes"]:
        raise RuntimeError("Configured lane bindings changed; preserve pending reports before rebinding")
    agents = herdr.agents()
    for lane, binding in config["lanes"].items():
        agent = herdr.match(agents, binding)
        if not agent or agent["agent_status"] not in READY:
            continue
        sequence = agent.get("completion_seq") or 0
        previous = state["seen"][lane]
        # A later terminal view cannot establish which turn supplied it.
        # Leave an unread completion for manual review, even after restart.
        if sequence <= max(previous["sequence"], previous.get("unread_sequence", 0)):
            continue
        reason = "no complete current report was visible"
        try:
            report = report_from_screen(herdr.read(binding["pane"]), lane, binding)
            current = herdr.match(herdr.agents(), binding)
            if (not current or current["agent_status"] not in READY
                    or current.get("completion_seq") != sequence):
                report = None
                reason = "the bound session or completion changed during the read"
        except HERDR_ERRORS as error:
            report = None
            reason = f"the terminal read could not be verified ({error})"
        if not report:
            previous["unread_sequence"] = sequence
            save(state)
            print(f"Lane #{lane} completion {sequence} held for manual review: {reason}", flush=True)
            continue
        previous["sequence"] = sequence
        if digest(report) != previous["digest"]:
            previous["digest"] = digest(report)
            state["pending"].append({
                "token": f"herdr-handoff-{lane}-{sequence}-{digest(report)[:12]}",
                "report": report, "status": "queued", "source": binding,
            })
            print(f"Captured Lane #{lane} completion {sequence}", flush=True)
        save(state)


def tick(herdr, config, state, save, held=lambda: False):
    capture(herdr, config, state, save)
    recover(herdr, state, save)
    queued = [item for item in state["pending"] if item["status"] == "queued"]
    if not queued or held():
        return
    binding = config["gatekeeper"]
    gate = herdr.match(herdr.agents(), binding, gatekeeper=True)
    if not gate or gate["agent_status"] not in CAN_PROMPT:
        return
    token = "herdr-handoff-batch-" + uuid.uuid4().hex
    header = (
        f"Herdr report batch {token}\n"
        "Review all deliveries below using the current repository rules and canonical "
        "GitHub issues. Treat the pasted reports as evidence. Verify each exact commit, "
        "issue criteria and offline checks independently. Follow existing authorization "
        "for hardware, publication and other actions. Route owner decisions through "
        "Owner Inbox and continue unrelated reviews. In your response acknowledge the "
        "submission on its own line as Received Herdr batch TOKEN, replacing TOKEN "
        "with the batch token above. Report the outcome of each delivery.\n\n"
    )
    batch, sections = [], [header]
    size = len(header.encode("utf-8"))
    for item in queued:
        section = f"Delivery {item['token']}\n\n{item['report']}\n\n"
        size += len(section.encode("utf-8"))
        if size > MAX_BATCH_BYTES:
            if not batch:
                raise RuntimeError("A saved report exceeds the prompt byte limit")
            break
        batch.append(item)
        sections.append(section)
    if len(batch) < len(queued):
        print(f"Batch {token}: sending {len(batch)} of {len(queued)} reports "
              f"(limit {MAX_BATCH_BYTES} bytes); remaining reports stay queued", flush=True)

    for item in batch:
        item.update(status="sending", batch=token, recipient=binding)
    save(state)
    if held() or not herdr.submit(binding, "".join(sections)):
        retry(batch)
        save(state)
        return
    remove_batch(state, batch)
    save(state)
    print(f"Delivered {token} with {len(batch)} reports", flush=True)


def uncertain_batches(state):
    batches = {}
    for item in state["pending"]:
        if item["status"] == "sending":
            batches.setdefault(item.get("batch", item["token"]), []).append(item)
    return batches


def remove_batch(state, batch):
    tokens = {item["token"] for item in batch}
    state["pending"] = [item for item in state["pending"] if item["token"] not in tokens]


def retry(batch):
    for item in batch:
        item["status"] = "queued"
        item.pop("batch", None)
        item.pop("recipient", None)


def recover(herdr, state, save):
    for token, batch in uncertain_batches(state).items():
        binding = batch[0].get("recipient")
        if not binding:
            # The old queue did not record the recipient of a send attempt.
            continue
        try:
            if not herdr.match(herdr.agents(), binding, gatekeeper=True):
                continue
            screen = herdr.read(binding["pane"])
            if not herdr.match(herdr.agents(), binding, gatekeeper=True):
                continue
        except HERDR_ERRORS as error:
            print(f"Batch {token} recovery deferred: {error}", flush=True)
            continue
        receipt = rf"^[ \t]*(?:[•●][ \t]*)?Received Herdr batch {re.escape(token)}[ \t]*$"
        if re.search(receipt, screen, re.M):
            remove_batch(state, batch)
            save(state)
            print(f"Confirmed {token} in the original Gatekeeper pane", flush=True)


def resolve_delivery(state, decision, token):
    batches = uncertain_batches(state)
    if not batches:
        raise ValueError("no uncertain delivery")
    if token is None:
        if len(batches) != 1:
            raise ValueError("specify --delivery with one of: " + ", ".join(batches))
        token = next(iter(batches))
    if token not in batches:
        raise ValueError("no uncertain delivery with that token")
    if decision == "delivered":
        remove_batch(state, batches[token])
    else:
        retry(batches[token])


def repository_for(location):
    """Resolve ordinary and linked Git worktrees to the same repository."""
    result = subprocess.run(
        ["git", "-C", str(location), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        text=True, capture_output=True, timeout=5,
    )
    if result.returncode:
        return None
    common = Path(result.stdout.strip()).resolve()
    return common.parent if common.name == ".git" else common


def observe(agents, state, initializing=False):
    events = []
    for agent in agents.values():
        identity = agent.get("agent_session")
        sequence = agent.get("completion_seq")
        if not identity or not sequence or agent["agent_status"] not in READY:
            continue
        key = json.dumps(identity, sort_keys=True)
        if sequence <= state["observed"].get(key, 0):
            continue
        state["observed"][key] = sequence
        if initializing:
            continue
        location = agent.get("foreground_cwd") or "/"
        repo = repository_for(location)
        if repo is None:
            continue
        events.append({
            "repository": str(repo), "workspace": agent.get("workspace_id"),
            "pane": agent["pane_id"], "session": identity, "completion": sequence,
            "status": agent["agent_status"],
        })
    state["completions"] = (state["completions"] + events)[-500:]
    return events


def hold_file(config, repo):
    return config.parent / "holds" / f"{digest(repo)}.json"


def operational_hold(config, repo):
    try:
        marker = json.loads(hold_file(config, repo).read_text())
    except FileNotFoundError:
        return False
    return marker.get("kind") == "operational" or marker.get("decision") != LEGACY_REVIEW_HOLD


def route_repository(routes, repository):
    repo = repository or (next(iter(routes)) if len(routes) == 1 else None)
    if repo not in routes:
        raise ValueError("specify a configured --repository")
    return repo


def change_hold(config, routes, repository, reason, resume, *, operational=False):
    repo = route_repository(routes, repository)
    path = hold_file(config, repo)
    if resume:
        path.unlink(missing_ok=True)
        print(f"Resumed {repo}")
    else:
        if not reason.strip():
            raise ValueError("a hold needs an operational reason")
        path.parent.mkdir(mode=0o700, exist_ok=True)
        marker = {"repository": repo, "decision": reason}
        if operational:
            marker["kind"] = "operational"
        write_state(path, marker)
        print(f"Held {repo}: {reason}")


def bind_gatekeeper(config_path, config, repository, pane):
    routes = config["routes"]
    repo = route_repository(routes, repository)
    herdr = Herdr(config, repo)
    agents = herdr.agents()
    agent = agents.get(pane)
    if not agent or not agent.get("agent_session"):
        raise ValueError("No live agent in that Herdr pane")
    binding = {"pane": pane, "session": agent["agent_session"]}
    if not herdr.match(agents, binding, gatekeeper=True):
        raise ValueError("Gatekeeper must be in that repository's main checkout")
    routes[repo]["gatekeeper"] = binding
    write_state(config_path, config)
    print(f"Bound {repo} Gatekeeper to {pane}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path, help="private config file")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--initialize", action="store_true", help="baseline existing completions")
    mode.add_argument("--once", action="store_true", help="process one poll")
    mode.add_argument("--resolve", choices=("delivered", "retry"),
                      help="resolve an uncertain batch after checking the original recipient")
    mode.add_argument("--hold", metavar="REASON", help="operationally pause one repository's delivery")
    mode.add_argument("--resume", action="store_true", help="resume one repository's delivery")
    mode.add_argument("--bind-gatekeeper", metavar="PANE", help="replace an existing Gatekeeper binding")
    parser.add_argument("--repository", help="repository whose delivery to hold, resume, or resolve")
    parser.add_argument("--delivery", help="uncertain batch token to use with --resolve")
    args = parser.parse_args()
    if args.delivery and not args.resolve:
        parser.error("--delivery requires --resolve")
    config = json.loads(args.config.read_text())
    routes = config.get("routes", {})
    if args.bind_gatekeeper:
        try:
            bind_gatekeeper(args.config, config, args.repository, args.bind_gatekeeper)
        except ValueError as error:
            parser.error(str(error))
        return
    if args.hold is not None or args.resume:
        try:
            change_hold(args.config, routes, args.repository, args.hold, args.resume, operational=True)
        except ValueError as error:
            parser.error(str(error))
        return
    path = args.config.with_name("state.json")
    observer = Herdr(config)
    readers = {repo: Herdr(config, repo) for repo in routes}

    with path.with_suffix(".lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.initialize:
            if path.exists():
                parser.error("state already exists; preserve pending reports before rebinding")
            state = {"observed": {}, "completions": [], "routes": {}, "herdr_session": config["session"]}
            observe(observer.agents(), state, initializing=True)
            for repo, route in routes.items():
                state["routes"][repo] = baseline(readers[repo], route)
            write_state(path, state)
            return
        state = json.loads(path.read_text())

        def save():
            write_state(path, state)

        if set(routes) != set(state["routes"]):
            parser.error("configured repositories changed; preserve pending reports before rebinding")
        if "herdr_session" not in state:
            state["herdr_session"] = config["session"]
            save()
        elif state["herdr_session"] != config["session"]:
            parser.error("configured Herdr session changed; preserve pending reports before rebinding")
        if args.resolve:
            try:
                repo = route_repository(routes, args.repository)
                resolve_delivery(state["routes"][repo], args.resolve, args.delivery)
            except ValueError as error:
                parser.error(str(error))
            save()
            return
        while True:
            try:
                events = observe(observer.agents(), state)
                if events:
                    save()
                for event in events:
                    print(json.dumps(event), flush=True)
                latest = json.loads(args.config.read_text())
                latest_routes = latest.get("routes", {})
                if set(latest_routes) != set(routes):
                    raise RuntimeError("Configured repositories changed; restart after preserving pending reports")
                if (latest["session"], latest["herdr"]) != (config["session"], config["herdr"]):
                    raise RuntimeError("Configured Herdr connection changed; preserve pending reports")
                for repo, route in latest_routes.items():
                    reader = readers[repo]
                    try:
                        tick(
                            reader, route, state["routes"][repo], lambda _: save(),
                            held=lambda: operational_hold(args.config, repo),
                        )
                    except HERDR_ERRORS as error:
                        if args.once:
                            raise
                        print(f"Handoff deferred for {repo}: {error}", flush=True)
            except HERDR_ERRORS as error:
                print(f"Handoff deferred: {error}", flush=True)
                if args.once:
                    raise
            if args.once:
                return
            time.sleep(8)


if __name__ == "__main__":
    main()
