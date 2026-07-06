# AI generated (Claude). Not mine!

"""
Sends OrderInsert commands to the exchange server every second, for pair (0, 1),
alternating between account 0 (always Ask) and account 1 (always Bid). Occasionally
sends an OrderCancel for a previously placed order instead of inserting.
Price is normally distributed around 100, volume is random.

Wire format (reverse-engineered from the Rust protocol):
  - Frame:   2-byte big-endian length prefix + MessagePack payload  (MpCommandEncoder)
  - Payload: CommandBuffer = array of Command
  - Command: externally-tagged enum -> {"OrderInsert": [fields...]} / {"OrderCancel": [fields...]}
             fields serialized positionally (rmp_serde's default "compact" struct mode)
  - Unit enum variants (OrderType::Limit, Side::Ask/Bid) -> plain strings
  - Price (I33F31, 31 fractional bits) -> the `fixed` crate serializes this as a
    1-element array containing the raw bits (value * 2**31). Confirmed against the
    sample bytes in the prompt: Price::ONE -> [2147483648] == [1 * 2**31].

Order ID tracking: this script ASSUMES it is the only client submitting orders, and
that the server assigns OrderId starting at 0 and incrementing by 1 for every
OrderInsert it processes. If another client is also submitting orders concurrently,
this local numbering will drift out of sync with the server's real IDs.

Requires: pip install msgpack
"""

import socket
import struct
import time
import random
import msgpack

HOST = "127.0.0.1"
PORT = 5555

PAIR = (0, 1)          # (primary, secondary) asset ids
PRICE_MEAN = 100.0
PRICE_STDDEV = 10.0     # adjust to taste
VOLUME_MIN = 5
VOLUME_MAX = 500
SEND_INTERVAL_SECONDS = 1.0

ORDERS_PER_SEND = 128
CANCEL_PROBABILITY = 0.2  # chance, each iteration, of attempting a cancel instead of an insert
MODIFY_PROBABILITY = 0.2  # chance, each iteration, of halving an existing order's volume

FRAC_BITS = 31          # I33F31 -> 31 fractional bits
PRICE_SCALE = 2 ** FRAC_BITS


def encode_price(value: float) -> list:
    """Encode a float as the raw fixed-point bits (I33F31), matching the
    `fixed` crate's non-human-readable msgpack representation: a 1-element array."""
    bits = int(round(value * PRICE_SCALE))
    return [bits]


def make_order_insert(account_id: int, side: str, price: float, volume: int) -> dict:
    primary, secondary = PAIR
    return {
        "OrderInsert": [
            account_id,           # account_id: u32
            "Limit",              # order_type: OrderType::Limit
            [primary, secondary], # pair: AssetIdPair { primary, secondary }
            side,                 # side: "Ask" | "Bid"
            volume,                # volume: u32
            encode_price(price),  # price: I33F31 -> [bits]
        ]
    }


def make_order_cancel(account_id: int, order_id: int) -> dict:
    return {
        "OrderCancel": [
            account_id,  # account_id: u32
            order_id,    # order_id: usize (plain int, not fixed-point, so unwrapped)
        ]
    }


def make_order_modify(order_id: int, account_id: int, new_volume: int) -> dict:
    return {
        "OrderModify": [
            order_id,    # order_id: usize
            account_id,  # account_id: u32
            new_volume,  # new_volume: Volume (u32)
        ]
    }


def encode_command_buffer(commands: list) -> bytes:
    """Pack a list of commands into a length-prefixed MessagePack frame,
    matching MpCommandEncoder: 2-byte big-endian length + msgpack bytes."""
    payload = msgpack.packb(commands, use_bin_type=True)
    if len(payload) > 0xFFFF:
        raise ValueError("Message too long!")
    return struct.pack(">H", len(payload)) + payload


def recv_exact(sock: socket.socket, n: int) -> bytes:
    """Read exactly n bytes from the socket, looping over recv() since a single
    call isn't guaranteed to return all of them. Raises ConnectionError if the
    peer closes the connection before n bytes arrive."""
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
    """Read one length-prefixed frame and return the raw MessagePack payload
    (undecoded) — mirrors the decoder side of the protocol: 2-byte big-endian
    length prefix, then that many bytes of MessagePack-encoded response."""
    length_bytes = recv_exact(sock, 2)
    (length,) = struct.unpack(">H", length_bytes)
    return recv_exact(sock, length)


def main():
    sock = socket.create_connection((HOST, PORT), timeout=5)
    sock.settimeout(5)  # so sendall() can't block forever if the server stops reading
    print(f"Connected to {HOST}:{PORT}")

    start = time.time()
    total_inserted = 0
    total_cancel_attempts = 0
    total_modify_attempts = 0

    next_order_id = 0          # client-side tracking of the next OrderId the server will assign
    open_orders = []           # list of dicts: {order_id, account_id, volume} for orders still tracked

    ask_turn = True  # alternate: True -> account 0 / Ask, False -> account 1 / Bid
    try:
        while True:
            roll = random.random()
            do_cancel = open_orders and roll < CANCEL_PROBABILITY
            do_modify = open_orders and CANCEL_PROBABILITY <= roll < CANCEL_PROBABILITY + MODIFY_PROBABILITY

            if do_cancel:
                idx = random.randrange(len(open_orders))
                order = open_orders.pop(idx)

                command = make_order_cancel(order["account_id"], order["order_id"])
                frame = encode_command_buffer([command])

                sock.sendall(frame)
                # print(f"Sent OrderCancel  account={order['account_id']}  order_id={order['order_id']}")

                total_cancel_attempts += 1
            elif do_modify:
                order = random.choice(open_orders)
                new_volume = order["volume"] // 2

                command = make_order_modify(order["order_id"], order["account_id"], new_volume)
                frame = encode_command_buffer([command])

                sock.sendall(frame)
                # print(f"Sent OrderModify  account={order['account_id']}  order_id={order['order_id']}  "
                    #   f"volume {order['volume']} -> {new_volume}")

                order["volume"] = new_volume
                total_modify_attempts += 1
            else:
                if ask_turn:
                    account_id, side = 0, "Ask"
                else:
                    account_id, side = 1, "Bid"

                commands = []
                batch_order_ids = []
                batch_volumes = []
                for _ in range(ORDERS_PER_SEND):
                    price = random.gauss(PRICE_MEAN, PRICE_STDDEV)
                    volume = random.randint(VOLUME_MIN, VOLUME_MAX)
                    commands.append(make_order_insert(account_id, side, price, volume))
                    batch_order_ids.append(next_order_id)
                    batch_volumes.append(volume)
                    next_order_id += 1

                frame = encode_command_buffer(commands)

                sock.sendall(frame)
                # print(f"Sent {side:<3} orders account={account_id}  order_ids={batch_order_ids}")

                open_orders.extend(
                    {"order_id": oid, "account_id": account_id, "volume": volume}
                    for oid, volume in zip(batch_order_ids, batch_volumes)
                )

                ask_turn = not ask_turn
                total_inserted += ORDERS_PER_SEND

            response_bytes = recv_frame(sock)
            # print(f"  -> raw response ({len(response_bytes)} bytes): {response_bytes}")

            # time.sleep(SEND_INTERVAL_SECONDS)
    except KeyboardInterrupt:
        elapsed = time.time() - start
        rate = total_inserted / elapsed if elapsed > 0 else 0.0
        print(f"\nRate: {rate:.2f}/s | Sent {total_inserted} orders, "
              f"{total_cancel_attempts} cancel attempts, {total_modify_attempts} modify attempts "
              f"in {elapsed:.1f}s")
        print("Stopping.")
    except socket.timeout:
        print("\nSocket timed out (server not reading/responding) — exiting.")
    except ConnectionError as e:
        print(f"\n{e}")
    finally:
        sock.close()


if __name__ == "__main__":
    main()