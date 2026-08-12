from enum import Enum
from dataclasses import dataclass
import sys

import random
import socket
import struct
import time

import msgpack


# Types
class OrderType(Enum):
    Limit = 0
    FillOrKill = 1
    Market = 2

class Side(Enum):
    Bid = 0
    Ask = 1

AssetId = int
AccountId = int
Volume = int
Price = int

@dataclass
class Asset:
    id: AssetId
    name: str
    symbol: str

@dataclass
class AssetPair():
    primary: Asset
    secondary: Asset

@dataclass
class AssetIdPair:
    primary: AssetId
    secondary: AssetId

@dataclass
class OrderInsertionRequest:
    # The id of the account that created the order.
    account_id: AccountId
    # The type of the order (limit, market, etc.). Field is `order_type` because `type` is a reserved keyword.
    order_type: OrderType
    # The pair that the order should be executed on.
    pair: AssetIdPair
    # Side of the order (Bid / Ask).
    side: Side
    # Volume of the order in whole units.
    volume: Volume
    # Price of the order.
    price: Price

FRACTIONAL_BITS = 31  # I33F31 -> 31 fractional bits
PRICE_SCALE = 2 ** FRACTIONAL_BITS

def encoded_price(f: float):
    return [round(f * PRICE_SCALE)]

class ExchangeClient():
    def __init__(self, exchange_addr, exchange_port=5555, autoconnect=True):
        self.exchange_addr = exchange_addr
        self.exchange_port = exchange_port
        self.autoconnect = autoconnect
        self.tcp_connection = None
        self.connect_retry()

    def connect_retry(self):
        if self.tcp_connection is not None:
            return
        while True:
            try:
                self.tcp_connection = socket.create_connection((self.exchange_addr, self.exchange_port), 5)
                print(f"Connected to {self.exchange_addr}:{self.exchange_port}.")
                return
            except (ConnectionRefusedError, socket.timeout, OSError) as e:
                print(f"Unable to connect to exchange server due to error: {e}. Retrying in 1 second.", file=sys.stderr)
                time.sleep(1)


def main():
    client = ExchangeClient("127.0.0.1")
    time.sleep(10)


if __name__ == "__main__":
    main()

    
