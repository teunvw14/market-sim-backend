# Quick and dirty script to automatically start the server every time the source
# code is changed.

import subprocess
from pathlib import Path
import glob
import os
import time
import sys


def main():
    # Run in release mode by default unless a --debug flag is given
    run_command = ["cargo","run", "--release"]
    if "--debug" in sys.argv:
        run_command = ["cargo","run"]

    proc = subprocess.Popen(run_command)
    files = list(Path("./src/").rglob("*.rs"))
    
    time_last_changes = {}
    # initialize values
    for file in files:
        time_last_changes[file] = os.path.getmtime(file)
    while True:
        for file in files:
            new_last_changed = os.path.getmtime(file)
            if new_last_changed > time_last_changes[file]:
                print("aaaaaa")
                time_last_changes[file] = new_last_changed
                proc.terminate()
                proc = subprocess.Popen(run_command)
        time.sleep(0.5)

if __name__ == "__main__":
    main()