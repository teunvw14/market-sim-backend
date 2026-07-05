import socket
import sys

def open_connection(host, port):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.connect((host, port))
        print(f"Connected to {host}:{port}")
        return sock
    except ConnectionRefusedError:
        print(f"Connection refused: nothing listening on {host}:{port}")
        sys.exit(1)
    except socket.timeout:
        print(f"Connection timed out: {host}:{port}")
        sys.exit(1)

if __name__ == "__main__":
    host = "127.0.0.1"
    port = 5555

    sock = open_connection(host, port)

    # Example: send and receive some data
    sock.sendall(b"hello\n")
    print("sent data")
    data = sock.recv(1024)
    print(f"Received: {data!r}")

    sock.close()