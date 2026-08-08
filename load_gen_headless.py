# AI generated (Claude). Not mine!

"""
Headless exchange load generator.

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

import random
import socket
import struct
import time

import msgpack

HOST = "127.0.0.1"
PORT = 5555

# --------------------------------------------------------------------------- #
# Tunables
# --------------------------------------------------------------------------- #

VOLUME_MEAN = 50.0
SEND_INTERVAL_SECONDS = 0.1
ORDERS_PER_SEND = 1
CANCEL_PROBABILITY = 0.05
MODIFY_PROBABILITY = 0.05
MARKET_ORDER_PROBABILITY = 0.05
FOK_ORDER_PROBABILITY = 0.05

STATUS_INTERVAL_SECONDS = 1.0

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


def connect_with_retry(host, port, retry_interval=5) -> socket.socket:
    while True:
        try:
            sock = socket.create_connection((host, port), timeout=5)
            print(f"Connected to {host}:{port}")
            return sock
        except (ConnectionRefusedError, socket.timeout, OSError) as e:
            print(f"Connection failed ({e}), retrying in {retry_interval}s...")
            time.sleep(retry_interval)


# --------------------------------------------------------------------------- #
# Sender loop
# --------------------------------------------------------------------------- #

def run():
    next_order_id = 0
    open_orders = []
    ask_turn = True
    sock = None

    inserted = cancel_attempts = modify_attempts = 0
    started_at = time.time()
    last_status_at = started_at

    try:
        while True:
            if sock is None:
                sock = connect_with_retry(HOST, PORT)
                sock.settimeout(5)

            try:
                action_roll = random.random()
                do_cancel = open_orders and action_roll < CANCEL_PROBABILITY
                do_modify = open_orders and (
                    CANCEL_PROBABILITY <= action_roll < CANCEL_PROBABILITY + MODIFY_PROBABILITY
                )

                # Separate roll: the order type must not be correlated with the
                # cancel/modify decision.
                type_roll = random.random()
                order_type = 0
                if type_roll < FOK_ORDER_PROBABILITY:
                    order_type = 1
                elif type_roll < FOK_ORDER_PROBABILITY + MARKET_ORDER_PROBABILITY:
                    order_type = 2

                if do_cancel:
                    order = open_orders.pop(random.randrange(len(open_orders)))
                    sock.sendall(encode_command_buffer(
                        [make_order_cancel(order["account_id"], order["order_id"])]
                    ))
                    cancel_attempts += 1
                elif do_modify:
                    order = random.choice(open_orders)
                    new_volume = order["volume"] // 2
                    sock.sendall(encode_command_buffer(
                        [make_order_modify(order["order_id"], order["account_id"], new_volume)]
                    ))
                    order["volume"] = new_volume
                    modify_attempts += 1
                else:
                    account_id, side = (0, 1) if ask_turn else (1, 0)

                    commands = []
                    new_orders = []
                    for _ in range(ORDERS_PER_SEND):
                        pair_idx = random.randrange(len(PAIRS))
                        price = max(0.001, random.gauss(PRICE_MEANS[pair_idx],
                                                        PRICE_STDDEVS[pair_idx]))
                        volume = max(1, int(random.expovariate(1 / VOLUME_MEAN)))
                        commands.append(make_order_insert(account_id, order_type, side,
                                                          price, volume, PAIRS[pair_idx]))
                        new_orders.append({"order_id": next_order_id,
                                           "account_id": account_id,
                                           "volume": volume})
                        next_order_id += 1

                    sock.sendall(encode_command_buffer(commands))
                    open_orders.extend(new_orders)
                    ask_turn = not ask_turn
                    inserted += ORDERS_PER_SEND

                recv_frame(sock)

                now = time.time()
                if now - last_status_at >= STATUS_INTERVAL_SECONDS:
                    elapsed = now - started_at
                    rate = inserted / elapsed if elapsed > 0 else 0.0
                    print(f"\r{rate:.1f} inserts/s | {inserted} inserted, "
                          f"{cancel_attempts} cancels, {modify_attempts} modifies",
                          end="", flush=True)
                    last_status_at = now

                if SEND_INTERVAL_SECONDS > 0:
                    time.sleep(SEND_INTERVAL_SECONDS)

            except (ConnectionError, socket.timeout, OSError) as e:
                print(f"\nConnection lost ({e}), will attempt to reconnect...")
                sock.close()
                sock = None

                inserted = cancel_attempts = modify_attempts = 0
                started_at = last_status_at = time.time()
                next_order_id = 0
                open_orders = []
                ask_turn = True
    except KeyboardInterrupt:
        print("\nShutting down.")
    finally:
        if sock is not None:
            sock.close()


if __name__ == "__main__":
    run()
