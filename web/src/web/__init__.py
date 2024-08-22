import os
import subprocess
import time
import fcntl
from flask import Flask, render_template, Response, json, request


app = Flask(__name__)

STDIN_FILE = os.environ.get("STDIN_FILE", "/tmp/teetty-stdin")
STDOUT_FILE = os.environ.get("STDOUT_FILE", "/tmp/teetty-stdout")


@app.route("/")
def index():
    return render_template("index.html")


@app.route("/input", methods=["POST"])
def input():
    text = request.json["input"]
    with open(STDIN_FILE, mode="w") as f:
        f.write(text)
        f.write("\n")
    return "OK"


@app.route("/stream")
def stream():
    child = subprocess.Popen(["tail", "-f", STDOUT_FILE], stdout=subprocess.PIPE)
    fd = child.stdout.raw.fileno()
    flag = fcntl.fcntl(fd, fcntl.F_GETFL)
    fcntl.fcntl(fd, fcntl.F_SETFL, flag | os.O_NONBLOCK)

    def generator():
        yield ("data:%s\n\n" % json.dumps({
            "version": 2,
            "width": 80,
            "height": 24,
            "timestamp": int(time.time()),
            "env": {"TERM": "dummy"}
        })).encode("utf-8")
        now = time.time()
        while True:
            line = child.stdout.raw.read(8000)
            if line:
                out = line.decode("utf-8", "replace")
                yield ("data:%s\n\n" % json.dumps([time.time() - now, "o", out])).encode("utf-8")
            else:
                time.sleep(0.1)

    return Response(generator(), mimetype="text/event-stream", direct_passthrough=True)
