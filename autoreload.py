# Quick and dirty script to automatically start the server every time the source
# code is changed.

import subprocess
from pathlib import Path
import glob
import os
import time
import sys

from dataclasses import dataclass
from typing import List

@dataclass
class Program():
    name: str
    run_command: List[str]
    run_command_debug: List[str]
    dependent_paths: List[str]
    process = None
    time_last_changes = {}

    def run(self, debug):
        if debug and self.run_command_debug is not None:
            self.process = subprocess.Popen(self.run_command_debug)
        else:
            self.process = subprocess.Popen(self.run_command)

    def restart(self, debug):
        if (self.process is not None):
            self.process.terminate()
        self.run(debug)
            

# The programs auto-reloaded anytime file changes occur 
# Format: [name, command, dev command (may be None), list of files]
PROGRAMS = [
    Program(
        "Exchange Server",
        ["cargo","run", "--release"],
        ["cargo", "run"],
        list(Path("./src/").rglob("*.rs")),
    ),
    Program(
        "Market Enforcers Script",
        ["python3", "market_enforcers.py"],
        None,
        [Path("market_enforcers.py")]
    )
]

def main():
    debug = False
    if "--debug" in sys.argv:
        debug = True
    
    # Run in release mode by default unless a --debug flag is given
    for program in PROGRAMS:
        program.run(debug)
        for file in program.dependent_paths:
            program.time_last_changes.update({ file: os.path.getmtime(file) })

    while True:
        for program in PROGRAMS:
            for file in program.dependent_paths:
                new_last_changed = os.path.getmtime(file)
                if new_last_changed > program.time_last_changes[file]:
                    print(f"[AUTORELOAD] Restarting {program.name} (change detected in {file}).")
                    program.time_last_changes[file] = new_last_changed
                    program.restart(debug)
        time.sleep(0.5)

if __name__ == "__main__":
    main()