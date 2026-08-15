import time
import random
from math import sqrt 
from dataclasses import dataclass

from python_client.client import ExchangeClient, OrderInsert
from python_client.exchange_types import Side, OrderType, AssetIdPair

client = ExchangeClient("127.0.0.1")

@dataclass
class CIRProcess():
    # Internal value
    value: float
    #Parameters
    S_0: float
    a: float
    b: float
    sigma: float

    def __init__(self, S_0, a, b, sigma):
        self.S_0 = S_0
        self.a = a
        self.b = b
        self.sigma = sigma
        self.value = S_0

    def update(self, dt):
        dWt = random.gauss(0, sqrt(dt))
        self.value = self.value + self.a * (self.b - self.value) * dt + self.sigma * sqrt(self.value) * dWt

# parameters
cir_eur_usd = CIRProcess(1.12, 0.01, 0.85, 1.0)
cir_jpy_usd = CIRProcess(0.006, 0.01, 0.85, 1.0)
cir_chf_usd = CIRProcess(1.23, 0.01, 0.85, 1.0)
cir_chf_eur = CIRProcess(1.06, 0.01, 0.85, 1.0)
cir_jpy_eur = CIRProcess(0.005, 0.01, 0.85, 1.0)

tps = 25
volume_mean = 100


rest = 1 / tps
start = time.time()
order_id = 0

while True:
    new_t = time.time()
    dt_seconds = new_t - start
    # 1 year = 256 trading days * 24 hours * 60 minutes * 60 seconds
    dt = dt_seconds / (60 * 60 * 24 * 256)
    cir_eur_usd.update(dt)

    side = Side.Bid
    if order_id % 2 == 0:
        side = Side.Ask
    cmd = OrderInsert(
        order_id % 2,
        OrderType.Limit,
        AssetIdPair(1, 0),
        side,
        int(random.expovariate(1/volume_mean)),
        cir_eur_usd.value
    )

    client.send_commands([cmd])

    t = new_t
    order_id += 1
    time.sleep(rest)