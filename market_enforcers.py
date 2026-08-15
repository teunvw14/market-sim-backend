from python_client.client import ExchangeClient, OrderInsert
from python_client.exchange_types import Side, OrderType, AssetIdPair
import time
import random
from math import sqrt 

client = ExchangeClient("127.0.0.1")

# parameters
S_0 = 0.85
a = 0.01
b = S_0
sigma = 10.0
tps = 25
volume_mean = 100


rest = 1 / tps
t = time.time()
S = S_0
order_id = 0

while True:
    new_t = time.time()
    dt_seconds = new_t - t
    # 1 year = 256 trading days * 24 hours * 60 minutes * 60 seconds
    dt = dt_seconds / (60 * 60 * 24 * 256)

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
        int(random.expovariate(1/volume_mean)),
        S
    )

    client.send_commands([cmd])

    t = new_t
    order_id += 1
    time.sleep(rest)