#!/usr/bin/env python3
"""Display an owner review and preserve the reply for the plan pane."""

import argparse
import json
import os
from pathlib import Path
import re
import select
import sys
import time
import uuid

from herdr_completion_handoff import change_hold, digest, route_repository, write_state


def review_root(config, repo):
    return config.parent / "reviews" / digest(repo)


def active_review(root):
    pointer = root / "active.json"
    if not pointer.exists():
        return None
    review_id = json.loads(pointer.read_text())["id"]
    if not re.fullmatch(r"[a-f0-9]{32}", review_id):
        raise ValueError("Invalid owner review identifier")
    return root / review_id


def open_review(config, routes, repo, text):
    root = review_root(config, repo)
    if active_review(root):
        raise ValueError("An owner review is already active for this repository")
    text = text.strip()
    if not text:
        raise ValueError("The owner needs a review request")
    path = root / uuid.uuid4().hex
    path.mkdir(mode=0o700, parents=True)
    write_state(path / "request.json", {"text": text, "repository": repo})
    change_hold(config, routes, repo, "Owner review in the dedicated review pane", False)
    write_state(root / "active.json", {"id": path.name})
    return path


def feedback(path):
    for file in sorted(path.glob("feedback-*.json")):
        yield file, json.loads(file.read_text())


def save_feedback(path, text):
    reply = text.strip()
    if not reply or len(reply) > 12_000:
        raise ValueError("Enter a comment between 1 and 12,000 characters")
    file = path / f"feedback-{time.time_ns()}-{uuid.uuid4().hex}.json"
    write_state(file, {"text": reply, "status": "queued"})
    return file


def summary(path):
    if path is None:
        return "No owner review is waiting."
    request = json.loads((path / "request.json").read_text())["text"]
    remarks = "\n".join(
        f"[{item['status']}] {item['text']}" for _, item in feedback(path)
    )
    return request + ("\n\nSaved feedback\n" + remarks if remarks else "")


def deliver_feedback(herdr, route, root):
    """Use a separate queue so feedback can reach a held review."""
    path = active_review(root)
    if path is None:
        return False
    item = next(((f, v) for f, v in feedback(path) if v["status"] != "delivered"), None)
    if item is None:
        return False
    binding = route["gatekeeper"]
    agent = herdr.match(herdr.agents(), binding, gatekeeper=True)
    if not agent or agent["agent_status"] not in {"idle", "done"}:
        return True
    file, reply = item
    token = file.stem
    if reply["status"] == "sending" and reply.get("recipient") == binding["session"]:
        if token not in herdr.read(binding["pane"]):
            return True
    else:
        reply["status"] = "sending"
        reply["recipient"] = binding["session"]
        write_state(file, reply)
        request = json.loads((path / "request.json").read_text())["text"]
        herdr.prompt(binding["pane"], (
            f"Owner review feedback {token} for this repository. Read the saved review "
            "request and feedback. Keep the lane hold while reviewing this response. "
            "Treat feedback as specific to this review and follow the project's usual "
            "authorization rules for any separate hardware or release decision.\n\n"
            f"Review request:\n{request}\n\nOwner feedback:\n{reply['text']}"
        ))
    reply["status"] = "delivered"
    write_state(file, reply)
    print(f"Delivered owner review feedback {token}", flush=True)
    return True


def review_pane(root):
    shown = None
    draft = []
    draft_review = None
    buffered = b""
    while True:
        path = active_review(root)
        display = summary(path)
        if display != shown and not draft:
            print("\x1b[2J\x1b[HOwner reviews\n=============\n\n" + display)
            if path:
                print("\nType your feedback below. Enter :send on its own line to submit.")
            sys.stdout.flush()
            shown = display
        if not select.select([sys.stdin], [], [], 1.0)[0]:
            continue
        chunk = os.read(sys.stdin.fileno(), 4096)
        if not chunk:
            return
        buffered += chunk
        while b"\n" in buffered:
            raw, buffered = buffered.split(b"\n", 1)
            line = raw.decode("utf-8", "replace").rstrip("\r")
            if path is None:
                continue
            if draft and draft_review != path:
                print("The review changed. Enter any new feedback for the request shown above.", flush=True)
                draft.clear()
                continue
            if line.strip() == ":send":
                try:
                    save_feedback(path, "\n".join(draft))
                except ValueError as error:
                    print(error, flush=True)
                    continue
                draft.clear()
                print("Feedback saved for the plan pane.", flush=True)
            else:
                draft_review = path
                draft.append(line)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    parser.add_argument("--repository")
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument("--open", type=Path, metavar="REQUEST")
    actions.add_argument("--pane", action="store_true")
    actions.add_argument("--show", action="store_true")
    actions.add_argument("--close", action="store_true")
    args = parser.parse_args()
    routes = json.loads(args.config.read_text())["routes"]
    try:
        repo = route_repository(routes, args.repository)
        root = review_root(args.config, repo)
        if args.open:
            print(open_review(args.config, routes, repo, args.open.read_text()))
        elif args.pane:
            review_pane(root)
        elif args.show:
            print(summary(active_review(root)))
        else:
            path = active_review(root)
            if path is None or not list(feedback(path)):
                raise ValueError("No owner feedback has been submitted")
            if any(item["status"] != "delivered" for _, item in feedback(path)):
                raise ValueError("Owner feedback is still awaiting delivery")
            (root / "active.json").unlink()
            print("Review archived. Resume handoffs separately after recording the outcome.")
    except ValueError as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
