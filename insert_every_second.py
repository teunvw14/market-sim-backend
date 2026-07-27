# AI generated (Claude). Not mine!

"""
Same load generator as before, but the tunable knobs live in a small Tkinter
window and are read fresh on every loop iteration, so changes take effect
immediately without restarting.

Wire format (reverse-engineered from the Rust protocol):
  - Frame:   2-byte big-endian length prefix + MessagePack payload  (MpCommandEncoder)
  - Payload: CommandBuffer = array of Command
  - Command: externally-tagged enum -> {"OrderInsert": [fields...]} / {"OrderCancel": [fields...]}
             fields serialized positionally (rmp_serde's default "compact" struct mode)
  - Unit enum variants (OrderType::Limit, Side::Ask/Bid) -> plain strings
  - Price (I33F31, 31 fractional bits) -> the `fixed` crate serializes this as a
    1-element array containing the raw bits (value * 2**31).

Order ID tracking: this script ASSUMES it is the only client submitting orders, and
that the server assigns OrderId starting at 0 and incrementing by 1 for every
OrderInsert it processes.

Requires: pip install msgpack
"""

import socket
import struct
import threading
import time
import random
import tkinter as tk
from dataclasses import dataclass

import msgpack

HOST = "127.0.0.1"
PORT = 5555

PAIRS = [
    [0, 1],
    [0, 2],
    [0, 3],
    [1, 2],
    [1, 3],
]  # (primary, secondary) asset id
PAIRS_NAMES = [
    "USD/EUR",
    "USD/JPY",
    "USD/CHF",
    "EUR/CHF",
    "EUR/JPY",
]
PRICE_MEANS = [0.87, 162.0, 0.81, 0.92, 186.0]
PRICE_STDDEVS = [0.05, 10, 0.03, 0.01, 12]

FRAC_BITS = 31  # I33F31 -> 31 fractional bits
PRICE_SCALE = 2 ** FRAC_BITS

ORDER_TYPES = {0: "Limit", 1: "FillOrKill", 2: "Market"}
SIDES = {0: "Bid", 1: "Ask"}


# --------------------------------------------------------------------------- #
# Live-tunable parameters
# --------------------------------------------------------------------------- #

@dataclass
class Params:
    """Written by the Tk thread, read by the sender thread. Attribute reads and
    writes of plain floats/ints are atomic under the GIL, so no lock is needed;
    the sender just picks up whatever the latest value is."""
    volume_mean: float = 50.0
    send_interval_seconds: float = 0.1
    orders_per_send: int = 1
    cancel_probability: float = 0.05
    modify_probability: float = 0.05
    market_order_probability: float = 0.05
    fok_order_probability: float = 0.05


@dataclass
class Stats:
    inserted: int = 0
    cancel_attempts: int = 0
    modify_attempts: int = 0
    connected: bool = False
    # Time accounting excludes paused periods, so the displayed rate stays
    # meaningful. `run_started_at` is 0.0 while paused.
    active_seconds: float = 0.0
    run_started_at: float = 0.0

    def elapsed(self) -> float:
        started = self.run_started_at
        extra = time.time() - started if started else 0.0
        return self.active_seconds + extra

    def reset(self):
        self.inserted = 0
        self.cancel_attempts = 0
        self.modify_attempts = 0
        self.active_seconds = 0.0
        self.run_started_at = time.time()


# --------------------------------------------------------------------------- #
# Protocol helpers
# --------------------------------------------------------------------------- #

def encode_price(value: float) -> list:
    """Encode a float as the raw fixed-point bits (I33F31), matching the
    `fixed` crate's non-human-readable msgpack representation: a 1-element array."""
    bits = int(round(value * PRICE_SCALE))
    return [bits]


def make_order_insert(account_id: int, order_type: int, side: int, price: float,
                      volume: int, pair: list) -> dict:
    return {
        "OrderInsert": [
            account_id,           # account_id: u32
            order_type,           # order_type
            pair,                 # pair: AssetIdPair { primary, secondary }
            side,                 # side: "Ask" (1) | "Bid" (0)
            volume,               # volume: u32
            encode_price(price),  # price: I33F31 -> [bits]
        ]
    }


def make_order_cancel(account_id: int, order_id: int) -> dict:
    return {"OrderCancel": [account_id, order_id]}


def make_order_modify(order_id: int, account_id: int, new_volume: int) -> dict:
    return {"OrderModify": [order_id, account_id, new_volume]}


def encode_command_buffer(commands: list) -> bytes:
    """Pack a list of commands into a length-prefixed MessagePack frame,
    matching MpCommandEncoder: 2-byte big-endian length + msgpack bytes."""
    payload = msgpack.packb(commands, use_bin_type=True)
    if len(payload) > 0xFFFF:
        raise ValueError("Message too long!")
    return struct.pack(">H", len(payload)) + payload


def recv_exact(sock: socket.socket, n: int) -> bytes:
    chunks = []
    remaining = n
    while remaining > 0:
        chunk = sock.recv(remaining)
        if not chunk:
            raise ConnectionError("Server closed the connection while reading a frame")
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def recv_frame(sock: socket.socket) -> bytes:
    length_bytes = recv_exact(sock, 2)
    (length,) = struct.unpack(">H", length_bytes)
    return recv_exact(sock, length)


def connect_with_retry(host, port, stop_event: threading.Event, retry_interval=5):
    while not stop_event.is_set():
        try:
            sock = socket.create_connection((host, port), timeout=5)
            print(f"Connected to {host}:{port}")
            return sock
        except (ConnectionRefusedError, socket.timeout, OSError) as e:
            print(f"Connection failed ({e}), retrying in {retry_interval}s...")
            stop_event.wait(retry_interval)
    return None


# --------------------------------------------------------------------------- #
# Sender loop (background thread)
# --------------------------------------------------------------------------- #

def sender_loop(params: Params, stats: Stats, stop_event: threading.Event,
                run_event: threading.Event):
    stats.run_started_at = time.time()

    next_order_id = 0
    open_orders = []
    ask_turn = True
    sock = None

    try:
        while not stop_event.is_set():
            if not run_event.is_set():
                # Freeze the clock, then idle until resumed (or shut down).
                stats.active_seconds = stats.elapsed()
                stats.run_started_at = 0.0
                while not run_event.wait(0.1):
                    if stop_event.is_set():
                        return
                stats.run_started_at = time.time()

            if sock is None:
                sock = connect_with_retry(HOST, PORT, stop_event)
                if sock is None:
                    break
                sock.settimeout(5)
                stats.connected = True

            try:
                action_roll = random.random()
                cancel_p = params.cancel_probability
                modify_p = params.modify_probability
                do_cancel = open_orders and action_roll < cancel_p
                do_modify = open_orders and cancel_p <= action_roll < cancel_p + modify_p

                # Separate roll: the order type must not be correlated with the
                # cancel/modify decision (see note below).
                type_roll = random.random()
                order_type = 0
                if type_roll < params.fok_order_probability:
                    order_type = 1
                elif type_roll < params.fok_order_probability + params.market_order_probability:
                    order_type = 2

                if do_cancel:
                    order = open_orders.pop(random.randrange(len(open_orders)))
                    frame = encode_command_buffer(
                        [make_order_cancel(order["account_id"], order["order_id"])]
                    )
                    sock.sendall(frame)
                    stats.cancel_attempts += 1
                elif do_modify:
                    order = random.choice(open_orders)
                    new_volume = order["volume"] // 2
                    frame = encode_command_buffer(
                        [make_order_modify(order["order_id"], order["account_id"], new_volume)]
                    )
                    sock.sendall(frame)
                    order["volume"] = new_volume
                    stats.modify_attempts += 1
                else:
                    account_id, side = (0, 1) if ask_turn else (1, 0)

                    orders_per_send = max(1, int(params.orders_per_send))
                    volume_mean = max(1.0, float(params.volume_mean))

                    commands = []
                    new_orders = []
                    for _ in range(orders_per_send):
                        pair_idx = random.randrange(len(PAIRS))
                        price = max(0.001, random.gauss(PRICE_MEANS[pair_idx],
                                                        PRICE_STDDEVS[pair_idx]))
                        volume = max(1, int(random.expovariate(1 / volume_mean)))
                        commands.append(make_order_insert(account_id, order_type, side,
                                                          price, volume, PAIRS[pair_idx]))
                        new_orders.append({"order_id": next_order_id,
                                           "account_id": account_id,
                                           "volume": volume})
                        next_order_id += 1

                    sock.sendall(encode_command_buffer(commands))
                    open_orders.extend(new_orders)
                    ask_turn = not ask_turn
                    stats.inserted += orders_per_send

                recv_frame(sock)

                interval = params.send_interval_seconds
                if interval > 0:
                    stop_event.wait(interval)

            except (ConnectionError, socket.timeout, OSError) as e:
                print(f"\nConnection lost ({e}), will attempt to reconnect...")
                sock.close()
                sock = None
                stats.connected = False
                stats.reset()

                next_order_id = 0
                open_orders = []
                ask_turn = True
    finally:
        if sock is not None:
            sock.close()
        stats.connected = False


# --------------------------------------------------------------------------- #
# GUI
# --------------------------------------------------------------------------- #

SLIDERS = [
    # (attribute, label, from, to, resolution)
    ("send_interval_seconds", "Send interval (s)", 0.0, 1.0, 0.005),
    ("orders_per_send", "Orders per send", 1, 100, 1),
    ("volume_mean", "Volume mean", 1, 500, 1),
    ("cancel_probability", "Cancel probability", 0.0, 1.0, 0.01),
    ("modify_probability", "Modify probability", 0.0, 1.0, 0.01),
    ("market_order_probability", "Market probability", 0.0, 1.0, 0.01),
    ("fok_order_probability", "FOK probability", 0.0, 1.0, 0.01),
]


def build_gui(params: Params, stats: Stats, stop_event: threading.Event,
              run_event: threading.Event):
    root = tk.Tk()
    root.title("Exchange load generator")

    for row, (attr, label, lo, hi, res) in enumerate(SLIDERS):
        tk.Label(root, text=label, anchor="w", width=20).grid(row=row, column=0,
                                                              sticky="w", padx=(8, 0))
        scale = tk.Scale(root, from_=lo, to=hi, resolution=res, orient="horizontal",
                         length=260,
                         command=lambda v, a=attr: setattr(params, a, float(v)))
        scale.set(getattr(params, attr))
        scale.grid(row=row, column=1, sticky="we", padx=(0, 8))

    toggle = tk.Button(root, text="Pause", width=10)
    toggle.grid(row=len(SLIDERS), column=0, columnspan=2, pady=(6, 0))

    def on_toggle():
        if run_event.is_set():
            run_event.clear()
            toggle.config(text="Resume")
        else:
            run_event.set()
            toggle.config(text="Pause")

    toggle.config(command=on_toggle)

    status = tk.Label(root, text="", anchor="w", justify="left")
    status.grid(row=len(SLIDERS) + 1, column=0, columnspan=2, sticky="we", padx=8, pady=(4, 8))

    def refresh():
        elapsed = stats.elapsed()
        rate = stats.inserted / elapsed if elapsed > 0 else 0.0
        if not run_event.is_set():
            state = "paused"
        else:
            state = "connected" if stats.connected else "disconnected"
        status.config(
            text=(f"{state} | {rate:.1f} inserts/s | {stats.inserted} inserted, "
                  f"{stats.cancel_attempts} cancels, {stats.modify_attempts} modifies")
        )
        root.after(250, refresh)

    def on_close():
        stop_event.set()
        run_event.set()  # wake the sender if it's idling in the pause loop
        root.destroy()

    root.protocol("WM_DELETE_WINDOW", on_close)
    refresh()
    return root


def main():
    params = Params()
    stats = Stats()
    stop_event = threading.Event()
    run_event = threading.Event()
    run_event.set()

    thread = threading.Thread(target=sender_loop,
                              args=(params, stats, stop_event, run_event),
                              daemon=True)
    thread.start()

    root = build_gui(params, stats, stop_event, run_event)
    try:
        root.mainloop()
    finally:
        stop_event.set()


if __name__ == "__main__":
    main()