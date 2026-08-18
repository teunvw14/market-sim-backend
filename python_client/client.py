import sys

import socket
import time

import msgpack
from dataclasses import dataclass

from python_client.command import OrderInsert, OrderModify, OrderCancel, GetBalance, GetOrderbookL1, GetOrderbookL2, GetAssets, GetAllOrderbookL1, decode_commands

MAX_CMD_BUF_SIZE = 1024

# Errors
class ServerClientError(Exception):
    pass

class ListTooLongError(ValueError, ServerClientError):
    pass

class EncodingError(ServerClientError):
    pass


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

    def recv_frame(self):
        '''
        Receive a frame and decode to a list of `CommandResult`s. Used for 
        reading responses.
        '''
        response_len_bytes = self.connection.recv(2)
        response_len = int.from_bytes(response_len_bytes, "big")
        response = self.connection.recv(response_len)
        response_decoded = msgpack.unpackb(response, object_hook=decode_commands)
        return response_decoded

    def send_command(self, command):
        ''''
        Wrapper around `send_commands` to send a single command.
        '''
        response = self.send_commands([command])
        if len(response) > 0:
            return response[0]
        return None

    def send_commands(self, commands):
        ''''
        Send a list of commands to the exchange, returning the parsed results.
        '''
        while True:
            try:
                if len(commands) > MAX_CMD_BUF_SIZE:
                    raise ListTooLongError

                # Encode the commands
                encoded_commands = [cmd.encode() for cmd in commands]
                encoded_commands_bytes: bytes = msgpack.packb(encoded_commands)

                # Messages are framed by starting each message with two bytes denoting 
                # the length of the coming frame
                length_commands = len(encoded_commands_bytes)
                if length_commands > 0xFFFF:
                    raise EncodingError

                message = length_commands.to_bytes(2, "big") + encoded_commands_bytes

                self.connection.sendall(message)
                response = self.recv_frame()
                return response
            except (ConnectionRefusedError, ConnectionResetError, socket.timeout, OSError) as e:
                time.sleep(1)
                self.reconnect()