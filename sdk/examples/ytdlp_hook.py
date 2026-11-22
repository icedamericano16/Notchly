"""yt-dlp progress → Notchly.  python ytdlp_hook.py <url>"""
import sys

import yt_dlp

sys.path.insert(0, __file__.rsplit("\\", 2)[0] + "\\python")
from notchly import Live  # noqa: E402

live = None


def hook(d):
    global live
    if d["status"] == "downloading":
        total = d.get("total_bytes") or d.get("total_bytes_estimate") or 0
        p = d["downloaded_bytes"] / total if total else None
        name = d.get("info_dict", {}).get("title", "Download")[:60]
        if live is None:
            live = Live("ytdlp", title=name, icon="download", priority=55)
        live.update(progress=p, subtitle=d.get("_speed_str", "").strip())
    elif d["status"] == "finished" and live:
        live.end("Downloaded")


with yt_dlp.YoutubeDL({"progress_hooks": [hook]}) as ydl:
    ydl.download(sys.argv[1:])
