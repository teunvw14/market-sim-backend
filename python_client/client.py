from enum import Enum
from dataclasses import dataclass, is_dataclass
import sys
from copy import deepcopy

import random
import socket
import struct
import time

import msgpack

MAX_CMD_BUF_SIZE = 1024

# Errors
class ServerClientError(Exception):
    pass

class ListTooLongError(ValueError, ServerClientError):
    pass

class EncodingError(ServerClientError):
    pass

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
OrderId = int

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

## Commands

# We define a generic Command type from which each specific Command type 
# (or really Command struct) will inherit
class Command():
    def encode(self):
        fields_as_dict = deepcopy(self.__dict__)
        # Encode fields named `price`
        if 'price' in fields_as_dict:
            fields_as_dict['price'] = encoded_price(fields_as_dict['price'])

        # Encode enums as just the underlying value and (data)classes as a tuple
        # of their fields
        for k, v in fields_as_dict.items():
            if isinstance(v, Enum):
                fields_as_dict[k] = v.value
            elif is_dataclass(v):
                fields_as_dict[k] = list(v.__dict__.values())

        fields_as_struct = list(fields_as_dict.values())
        return {
            f"{type(self).__name__}": fields_as_struct
        }

# We define a generic CommandResult type from which each specific CommandResult
# type will inherit
class CommandResult():
    pass

@dataclass
class OrderInsert(Command):
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

@dataclass
class OrderCancel(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to cancel
    order_id: OrderId

@dataclass
class OrderModify(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to modify
    order_id: OrderId
    # The new volume for the order. Must be lower than original.
    new_volume: Volume

@dataclass
class GetBalance(Command):
    # The id of the account that created the order.
    account_id: AccountId
    # The id of the order to modify
    asset_id: AssetId

@dataclass
class GetOrderBookL1(Command):
    pair: AssetIdPair

@dataclass
class GetOrderBookL2(Command):
    pair: AssetIdPair

@dataclass
class GetAssets(Command):
    pass

@dataclass
class GetAllOrderbookL1(Command):
    pass


FRACTIONAL_BITS = 31  # I33F31 -> 31 fractional bits
PRICE_SCALE = 2 ** FRACTIONAL_BITS

# Encodes the price as a fixed point integer
def encoded_price(f: float):
    return [round(f * PRICE_SCALE)]

def socket_is_open(sock: socket.socket):
    try:
        data = sock.recv(1, socket.MSG_PEEK | socket.MSG_DONTWAIT)
        if len(data) == 0:
            return False
    except BlockingIOError:
        return True # socket is open; reading from it would block
    except ConnectionResetError:
        return False # socket was closed for some reason
    except Exception as e:
        return True # Other exception was raised, unrelated to the connection
    return True

class ExchangeClient():
    def __init__(self, exchange_addr, exchange_port=5555, autoconnect=True):
        self.exchange_addr = exchange_addr
        self.exchange_port = exchange_port
        self.autoconnect = autoconnect
        self.connection = None
        self.connect_with_retry()

    def connect_with_retry(self):
        if self.connection is not None:
            return
        while True:
            try:
                self.connection = socket.create_connection((self.exchange_addr, self.exchange_port), 5)
                print(f"Connected to {self.exchange_addr}:{self.exchange_port}.")
                return
            except (ConnectionRefusedError, socket.timeout, OSError) as e:
                print(f"Unable to connect to exchange server due to error: {e}. Retrying in 1 second.", file=sys.stderr)
                time.sleep(1)

    def reconnect(self):
        if self.connection is not None and self.autoconnect:
            if not socket_is_open(self.connection):
                print("Connection to server broken. Trying to reconnect...")
                self.connect_with_retry()

    def send_commands(self, commands: list[Command]):
        if len(commands) > MAX_CMD_BUF_SIZE:
            raise ListTooLongError

        # Encode the commands
        encoded_commands = [cmd.encode() for cmd in commands]
        encoded_commands_bytes: bytes = msgpack.packb(encoded_commands, use_bin_type=True)

        # Messages are framed by starting each message with two bytes denoting 
        # the length of the coming frame
        length_commands = len(encoded_commands_bytes)
        if length_commands > 0xFFFF:
            raise EncodingError

        message = length_commands.to_bytes(2, "big") + encoded_commands_bytes
        print(f"sending message:\n{message}")

        self.connection.sendall(message)

