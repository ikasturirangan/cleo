#!/usr/bin/env python3
"""MJPEG HTTP stream from Pi camera using rpicam-vid (no extra packages needed).

Streams at http://0.0.0.0:8080/stream.mjpg
"""
import os
import subprocess
import threading
from http import server

PORT   = int(os.environ.get("SLITCAM_STREAM_PORT",   "8080"))
WIDTH  = int(os.environ.get("SLITCAM_STREAM_WIDTH",  "640"))
HEIGHT = int(os.environ.get("SLITCAM_STREAM_HEIGHT", "480"))
FPS    = int(os.environ.get("SLITCAM_STREAM_FPS",    "30"))


class FrameBuffer:
    """Holds the latest JPEG frame; notifies waiting HTTP handlers."""
    def __init__(self):
        self.frame = None
        self.condition = threading.Condition()

    def put(self, frame: bytes):
        with self.condition:
            self.frame = frame
            self.condition.notify_all()

    def get(self):
        with self.condition:
            self.condition.wait()
            return self.frame


buf = FrameBuffer()


def capture_loop():
    """Run rpicam-vid and split its raw MJPEG output into individual frames."""
    cmd = [
        "rpicam-vid",
        "--codec", "mjpeg",
        "--width",  str(WIDTH),
        "--height", str(HEIGHT),
        "--framerate", str(FPS),
        "--nopreview",
        "-t", "0",   # run forever
        "-o", "-",   # stdout
    ]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    data = b""
    while True:
        chunk = proc.stdout.read(65536)
        if not chunk:
            break
        data += chunk
        # Split on JPEG SOI/EOI boundaries (0xFF 0xD8 ... 0xFF 0xD9)
        while True:
            soi = data.find(b"\xff\xd8")
            if soi == -1:
                data = b""
                break
            eoi = data.find(b"\xff\xd9", soi + 2)
            if eoi == -1:
                data = data[soi:]   # keep partial frame for next read
                break
            buf.put(data[soi : eoi + 2])
            data = data[eoi + 2:]


class StreamHandler(server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/stream.mjpg":
            self.send_response(200)
            self.send_header("Age", "0")
            self.send_header("Cache-Control", "no-cache, private")
            self.send_header("Pragma", "no-cache")
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header(
                "Content-Type",
                "multipart/x-mixed-replace; boundary=FRAME",
            )
            self.end_headers()
            try:
                while True:
                    frame = buf.get()
                    self.wfile.write(
                        b"--FRAME\r\n"
                        b"Content-Type: image/jpeg\r\n"
                        + f"Content-Length: {len(frame)}\r\n\r\n".encode()
                        + frame
                        + b"\r\n"
                    )
            except Exception:
                pass
        elif self.path == "/healthz":
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"ok")
        else:
            self.send_error(404)

    def log_message(self, fmt, *args):
        pass


t = threading.Thread(target=capture_loop, daemon=True)
t.start()

print(
    f"[camera-stream] MJPEG at http://0.0.0.0:{PORT}/stream.mjpg "
    f"({WIDTH}x{HEIGHT} @ {FPS}fps)",
    flush=True,
)

httpd = server.HTTPServer(("", PORT), StreamHandler)
httpd.serve_forever()
