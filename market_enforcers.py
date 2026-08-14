from python_client.client import ExchangeClient, OrderInsert, OrderType, Side, AssetIdPair
import time
import random
from math import sqrt 

client = ExchangeClient("127.0.0.1")

S_0 = 0.86
a = 0.01
b = S_0
sigma = 0.03

tps = 250
rest = 1 / tps

t = time.time()

S = S_0
order_id = 0
while True:
    new_t = time.time()
    dt = new_t - t

    dWt = random.gauss(0, sqrt(dt))
    S = S + a * (b - S) * dt + sigma * sqrt(S) * dWt

    side = Side.Bid
    if order_id % 2 == 0:
        side = Side.Ask
    cmd = OrderInsert(
        order_id % 2,
        OrderType.Limit,
        AssetIdPair(0, 1),
        side,
        10,
        S
    )

    client.send_commands([cmd])

    t = new_t
    order_id += 1
    time.sleep(rest)