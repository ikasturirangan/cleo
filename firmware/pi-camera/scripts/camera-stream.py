#!/usr/bin/env python3
"""MJPEG HTTP stream from Pi camera using picamera2 (libcamera).

Streams at http://0.0.0.0:8080/stream.mjpg
"""
import io
import os
import sys
import threading
from http import server

from picamera2 import Picamera2
from picamera2.encoders import MJPEGEncoder
from picamera2.outputs import FileOutput

PORT    = int(os.environ.get("SLITCAM_STREAM_PORT", "8080"))
WIDTH   = int(os.environ.get("SLITCAM_STREAM_WIDTH", "640"))
HEIGHT  = int(os.environ.get("SLITCAM_STREAM_HEIGHT", "480"))
FPS     = int(os.environ.get("SLITCAM_STREAM_FPS", "30"))


class StreamingOutput(io.BufferedIOBase):
    def __init__(self):
        self.frame = None
        self.condition = threading.Condition()

    def write(self, buf):
        with self.condition:
            self.frame = buf
            self.condition.notify_all()
        return len(buf)


class StreamingHandler(server.BaseHTTPRequestHandler):
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
                    with output.condition:
                        output.condition.wait()
                        frame = output.frame
                    self.wfile.write(b"--FRAME\r\n")
                    self.send_header("Content-Type", "image/jpeg")
                    self.send_header("Content-Length", str(len(frame)))
                    self.end_headers()
                    self.wfile.write(frame)
                    self.wfile.write(b"\r\n")
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
        pass  # suppress per-request access logs


output = StreamingOutput()

picam2 = Picamera2()
config = picam2.create_video_configuration(
    main={"size": (WIDTH, HEIGHT), "format": "RGB888"},
    controls={"FrameRate": FPS},
)
picam2.configure(config)
picam2.start_recording(MJPEGEncoder(), FileOutput(output))

print(
    f"[camera-stream] MJPEG stream at http://0.0.0.0:{PORT}/stream.mjpg "
    f"({WIDTH}x{HEIGHT} @ {FPS}fps)",
    flush=True,
)

try:
    httpd = server.HTTPServer(("", PORT), StreamingHandler)
    httpd.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    picam2.stop_recording()
    sys.exit(0)
