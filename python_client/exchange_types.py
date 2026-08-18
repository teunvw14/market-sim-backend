from dataclasses import dataclass
from enum import Enum

# Types

# Wrapper Types

@dataclass(frozen=True, slots=True)
class Ok[T]:
    value: T

@dataclass(frozen=True, slots=True)
class Err[E]:
    value: E

type Result[T, E] = Ok[T] | Err[E]

def is_err(obj):
    return isinstance(obj, Err)

def is_ok(obj):
    return isinstance(obj, Ok)


# Other Types

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
Price = float
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

class OrderExecutionStatus(Enum):
    AwaitingFill = 0
    PartialFill = 1
    Filled = 2
    Killed = 3
    Cancelled = 4
