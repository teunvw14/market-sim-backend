import time
import random
from math import sqrt 
from dataclasses import dataclass, field

from python_client.client import ExchangeClient, OrderInsert
from python_client.exchange_types import Side, OrderType, AssetIdPair


TPS = 50
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
class GBMProcess():
    '''Simple GBM process.'''
    # Internal value
    value: float
    #Parameters
    S_0: float
    mu: float
    sigma: float

    def __init__(self, S_0, mu, sigma):
        self.S_0 = S_0
        self.mu = mu
        self.sigma = sigma
        self.value = S_0

    def update(self, dt):
        dWt = random.gauss(0, sqrt(dt))
        self.value = self.value * (1 + self.mu * dt + self.sigma * dWt)

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
        CIRProcess(1.12, 1.0, 1.12, 10.0),
    ),
    MarketEnforcer( # JPY/USD
        AssetIdPair(2, 0),
        CIRProcess(0.0063, 1.0, 0.0063, 5.0),
    ),
    MarketEnforcer( # CHF/USD
        AssetIdPair(3, 0),
        CIRProcess(1.23, 1.0, 1.23, 10.0),
    ),
    MarketEnforcer( # JPY/EUR
        AssetIdPair(2, 1),
        CIRProcess(0.0054, 1.0, 0.0054, 5.0),
    ),
    MarketEnforcer( # CHF/EUR
        AssetIdPair(3, 1),
        CIRProcess(1.06, 1.0, 1.06, 5.0),
    ),
    MarketEnforcer( # SKHY/USD
        AssetIdPair(4, 0),
        GBMProcess(163, 0.4, 7.0)
    ),
    MarketEnforcer( # ADYEN.AS/EUR
        AssetIdPair(5, 1),
        GBMProcess(1062, 0.1, 2.0)
    ),
    MarketEnforcer( # NVDA/USD
        AssetIdPair(6, 0),
        GBMProcess(216, 0.5, 4.0)
    ),
    MarketEnforcer( # ASML/EUR
        AssetIdPair(7, 1),
        GBMProcess(1501, 0.2, 4.0)
    ),
    MarketEnforcer( # ASML/EUR
        AssetIdPair(8, 0),
        GBMProcess(463.2, 0.1, 2.0)
    ),
]


client = ExchangeClient("127.0.0.1")
iteration = 0

while True:
    enforcer = ENFORCERS[iteration % len(ENFORCERS)]
    enforcer.do_trade(client)

    iteration += 1
    time.sleep(REST)