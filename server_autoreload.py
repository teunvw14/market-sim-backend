import subprocess
from pathlib import Path
import glob
import os
import time

def main():
    proc = subprocess.Popen(["cargo","run", "--release"])
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
                proc = subprocess.Popen(["cargo","run", "--release"])
        time.sleep(0.5)

if __name__ == "__main__":
    main()