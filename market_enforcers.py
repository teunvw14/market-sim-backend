import time
import random
from math import sqrt 
from dataclasses import dataclass, field

from python_client.client import ExchangeClient, OrderInsert
from python_client.exchange_types import Side, OrderType, AssetIdPair


TPS = 25
REST = 1 / TPS
VOLUME_MEAN = 100


@dataclass
class CIRProcess():
    '''Reflecting CIR process.'''
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
        self.value = abs(self.value + self.a * (self.b - self.value) * dt + self.sigma * sqrt(self.value) * dWt)

@dataclass
class MarketEnforcer():
    market: AssetIdPair
    price_process: CIRProcess
    _open_orders: list[int] = field(default_factory=list)
    last_trade_timestamp: int = time.time()
    _local_iteration: int = 0

    def do_trade(self, client):
        new_time = time.time()
        dt_seconds = new_time - self.last_trade_timestamp
        self.last_trade_timestamp = new_time
        # 1 year = 256 trading days * 24 hours * 60 minutes * 60 seconds
        dt = dt_seconds / (60 * 60 * 24 * 256)
        self.price_process.update(dt)

        side = Side.Bid
        if self._local_iteration % 2 == 0:
            side = Side.Ask
        cmd = OrderInsert(
            self._local_iteration % 2,
            OrderType.Limit,
            self.market, 
            side,
            int(random.expovariate(1/VOLUME_MEAN)),
            self.price_process.value
        )

        client.send_command(cmd)

        self._local_iteration += 1


# enforcers
ENFORCERS = [
    MarketEnforcer( # EUR/USD
        AssetIdPair(1, 0),
        CIRProcess(1.12, 0.1, 1.12, 10.0),
    ),
    MarketEnforcer( # JPY/USD
        AssetIdPair(2, 0),
        CIRProcess(0.0063, 0.1, 0.0063, 20.0),
    ),
    MarketEnforcer( # CHF/USD
        AssetIdPair(3, 0),
        CIRProcess(1.23, 0.1, 1.23, 10.0),
    ),
    MarketEnforcer( # JPY/EUR
        AssetIdPair(2, 1),
        CIRProcess(0.0054, 0.1, 0.0054, 20.0),
    ),
    MarketEnforcer( # CHF/EUR
        AssetIdPair(3, 1),
        CIRProcess(1.06, 0.1, 1.06, 5.0),
    ),
]


client = ExchangeClient("127.0.0.1")
iteration = 0

while True:
    enforcer = ENFORCERS[iteration % len(ENFORCERS)]
    enforcer.do_trade(client)

    iteration += 1
    time.sleep(REST)