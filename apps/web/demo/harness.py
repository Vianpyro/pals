# Runs a program as its own process would, in the interpreter python.worker.ts keeps between runs:
# its own globals and standard streams, and a success like an exit status of 0.
# shortcut: what bypasses sys.stdout (os.write) is not captured; no exercise needs it so far
import io
import json
import sys


def run(source, stdin):
    streams = sys.stdin, sys.stdout, sys.stderr, sys.argv
    sys.stdin = io.TextIOWrapper(io.BytesIO(stdin.encode()), encoding="utf-8")
    # Held until read: a wrapper that is garbage collected closes the bytes under it
    stdout = io.TextIOWrapper(io.BytesIO(), encoding="utf-8", write_through=True)
    sys.stdout = stdout
    sys.stderr = io.StringIO()
    sys.argv = ["main.py"]
    try:
        exec(compile(source, "main.py", "exec"), {"__name__": "__main__"})
        success = True
    except SystemExit as exit:
        success = exit.code is None or exit.code == 0
    except BaseException:
        success = False
    finally:
        sys.stdin, sys.stdout, sys.stderr, sys.argv = streams
    return json.dumps([success, stdout.buffer.getvalue().decode(errors="replace")])


run
