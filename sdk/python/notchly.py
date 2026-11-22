"""Notchly Live Activities SDK for Python (Windows). No dependencies.

    from notchly import Live
    with Live("build-42", title="Build", icon="hammer") as a:
        a.update(progress=0.5, subtitle="linking")
        a.end("Done in 2m 14s")
"""
import json
import threading

PIPE = r"\\.\pipe\notchly-live"


class Live:
    def __init__(self, id, on_action=None, **fields):
        self.id = id
        self._f = open(PIPE, "r+b", buffering=0)  # raises if Notchly isn't running
        self._send({"op": "start", "id": id, **fields})
        if on_action:
            threading.Thread(target=self._listen, args=(on_action,), daemon=True).start()

    def _send(self, msg):
        self._f.write((json.dumps(msg) + "\n").encode())

    def _listen(self, cb):
        for line in iter(self._f.readline, b""):
            ev = json.loads(line)
            if ev.get("event") == "action":
                cb(ev["action"])

    def update(self, **fields):
        self._send({"op": "update", "id": self.id, **fields})

    def end(self, final="Done", linger=4):
        self._send({"op": "end", "id": self.id, "final": final, "linger": linger})

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self._f.close()
